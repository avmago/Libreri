//! The pix2tex model (Lukas Blecher, MIT): a ResNet + vision transformer
//! encoder reads the picture, and a transformer decoder writes LaTeX
//! tokens one at a time. Weights are the published `weights.pth`.

use candle_core::{DType, Device, Module, Result, Tensor, D};
use candle_nn::{GroupNorm, LayerNorm, Linear, VarBuilder};

/// Settings of the published model (`config.yaml`).
pub const DIM: usize = 256;
const HEADS: usize = 8;
const ENCODER_DEPTH: usize = 4;
const DECODER_DEPTH: usize = 4;
const BACKBONE: [usize; 3] = [2, 3, 7];
const MAX_WIDTH: usize = 672;
const PATCH: usize = 16;
pub const MAX_SEQ: usize = 512;
pub const BOS: u32 = 1;
pub const EOS: u32 = 2;

/// "Same" padding for a kernel and stride (as TensorFlow does it).
fn pad_same(x: &Tensor, k: usize, s: usize, value: f32) -> Result<Tensor> {
    let (_, _, h, w) = x.dims4()?;
    let total = |i: usize| {
        let out = i.div_ceil(s);
        ((out - 1) * s + k).saturating_sub(i)
    };
    let (ph, pw) = (total(h), total(w));
    if ph == 0 && pw == 0 {
        return Ok(x.clone());
    }
    let pad = |t: &Tensor, dim: usize, before: usize, after: usize| -> Result<Tensor> {
        if value == 0.0 {
            t.pad_with_zeros(dim, before, after)
        } else {
            let mut shape = t.dims().to_vec();
            let mut parts = Vec::new();
            if before > 0 {
                shape[dim] = before;
                parts.push(Tensor::full(value, shape.as_slice(), t.device())?);
            }
            parts.push(t.clone());
            if after > 0 {
                shape[dim] = after;
                parts.push(Tensor::full(value, shape.as_slice(), t.device())?);
            }
            Tensor::cat(&parts, dim)
        }
    };
    let x = pad(x, 2, ph / 2, ph - ph / 2)?;
    pad(&x, 3, pw / 2, pw - pw / 2)
}

/// A convolution with weight standardisation and "same" padding (timm's
/// `StdConv2dSame`).
struct StdConv {
    weight: Tensor,
    bias: Option<Tensor>,
    k: usize,
    stride: usize,
}

impl StdConv {
    fn new(vb: VarBuilder, cin: usize, cout: usize, k: usize, stride: usize) -> Result<Self> {
        let w = vb.get((cout, cin, k, k), "weight")?;
        // Standardise each output filter (biased variance, eps 1e-6).
        let flat = w.reshape((cout, cin * k * k))?;
        let mean = flat.mean_keepdim(1)?;
        let centred = flat.broadcast_sub(&mean)?;
        let var = centred.sqr()?.mean_keepdim(1)?;
        let weight = centred
            .broadcast_div(&(var + 1e-6)?.sqrt()?)?
            .reshape((cout, cin, k, k))?;
        Ok(Self {
            weight,
            bias: None,
            k,
            stride,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = pad_same(x, self.k, self.stride, 0.0)?;
        let y = x.conv2d(&self.weight, 0, self.stride, 1, 1)?;
        match &self.bias {
            Some(b) => y.broadcast_add(&b.reshape((1, (), 1, 1))?),
            None => Ok(y),
        }
    }
}

struct Norm {
    gn: GroupNorm,
    act: bool,
}

impl Norm {
    fn new(vb: VarBuilder, c: usize, act: bool) -> Result<Self> {
        Ok(Self {
            gn: candle_nn::group_norm(32, c, 1e-5, vb)?,
            act,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let y = self.gn.forward(x)?;
        if self.act {
            y.relu()
        } else {
            Ok(y)
        }
    }
}

/// timm's non-pre-activation bottleneck block.
struct Bottleneck {
    down: Option<(StdConv, Norm)>,
    conv1: StdConv,
    norm1: Norm,
    conv2: StdConv,
    norm2: Norm,
    conv3: StdConv,
    norm3: Norm,
}

impl Bottleneck {
    fn new(vb: VarBuilder, cin: usize, cout: usize, stride: usize, first: bool) -> Result<Self> {
        let mid = cout / 4;
        let down = if first {
            Some((
                StdConv::new(vb.pp("downsample.conv"), cin, cout, 1, stride)?,
                Norm::new(vb.pp("downsample.norm"), cout, false)?,
            ))
        } else {
            None
        };
        Ok(Self {
            down,
            conv1: StdConv::new(vb.pp("conv1"), cin, mid, 1, 1)?,
            norm1: Norm::new(vb.pp("norm1"), mid, true)?,
            conv2: StdConv::new(vb.pp("conv2"), mid, mid, 3, stride)?,
            norm2: Norm::new(vb.pp("norm2"), mid, true)?,
            conv3: StdConv::new(vb.pp("conv3"), mid, cout, 1, 1)?,
            norm3: Norm::new(vb.pp("norm3"), cout, false)?,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let shortcut = match &self.down {
            Some((c, n)) => n.forward(&c.forward(x)?)?,
            None => x.clone(),
        };
        let y = self.norm1.forward(&self.conv1.forward(x)?)?;
        let y = self.norm2.forward(&self.conv2.forward(&y)?)?;
        let y = self.norm3.forward(&self.conv3.forward(&y)?)?;
        (y + shortcut)?.relu()
    }
}

struct Backbone {
    stem: StdConv,
    stem_norm: Norm,
    blocks: Vec<Bottleneck>,
}

impl Backbone {
    fn new(vb: VarBuilder) -> Result<Self> {
        let stem = StdConv::new(vb.pp("stem.conv"), 1, 64, 7, 2)?;
        let stem_norm = Norm::new(vb.pp("stem.norm"), 64, true)?;
        let mut blocks = Vec::new();
        let mut cin = 64;
        for (i, &depth) in BACKBONE.iter().enumerate() {
            let cout = 256 << i;
            for b in 0..depth {
                let stride = if i > 0 && b == 0 { 2 } else { 1 };
                blocks.push(Bottleneck::new(
                    vb.pp(format!("stages.{i}.blocks.{b}")),
                    cin,
                    cout,
                    stride,
                    b == 0,
                )?);
                cin = cout;
            }
        }
        Ok(Self {
            stem,
            stem_norm,
            blocks,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.stem_norm.forward(&self.stem.forward(x)?)?;
        // Max pooling 3×3, stride 2, "same" (values are ≥ 0 after ReLU,
        // so padding with zeros is the same as with -∞).
        let mut x = pad_same(&x, 3, 2, 0.0)?.max_pool2d_with_stride(3, 2)?;
        for b in &self.blocks {
            x = b.forward(&x)?;
        }
        Ok(x)
    }
}

fn gelu(x: &Tensor) -> Result<Tensor> {
    x.gelu_erf()
}

/// Multi-head self-attention of the vision transformer.
struct VitAttention {
    qkv: Linear,
    proj: Linear,
}

impl VitAttention {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let (b, n, c) = x.dims3()?;
        let hd = c / HEADS;
        let qkv = self
            .qkv
            .forward(x)?
            .reshape((b, n, 3, HEADS, hd))?
            .permute((2, 0, 3, 1, 4))?;
        let q = qkv.get(0)?.contiguous()?;
        let k = qkv.get(1)?.contiguous()?;
        let v = qkv.get(2)?.contiguous()?;
        let att = (q.matmul(&k.t()?)? * (hd as f64).powf(-0.5))?;
        let att = candle_nn::ops::softmax_last_dim(&att)?;
        let out = att.matmul(&v)?.transpose(1, 2)?.reshape((b, n, c))?;
        self.proj.forward(&out)
    }
}

struct VitBlock {
    norm1: LayerNorm,
    attn: VitAttention,
    norm2: LayerNorm,
    fc1: Linear,
    fc2: Linear,
}

impl VitBlock {
    fn new(vb: VarBuilder) -> Result<Self> {
        let ln = |p: &str| candle_nn::layer_norm(DIM, 1e-6, vb.pp(p));
        Ok(Self {
            norm1: ln("norm1")?,
            attn: VitAttention {
                qkv: candle_nn::linear(DIM, DIM * 3, vb.pp("attn.qkv"))?,
                proj: candle_nn::linear(DIM, DIM, vb.pp("attn.proj"))?,
            },
            norm2: ln("norm2")?,
            fc1: candle_nn::linear(DIM, DIM * 4, vb.pp("mlp.fc1"))?,
            fc2: candle_nn::linear(DIM * 4, DIM, vb.pp("mlp.fc2"))?,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = (x + self.attn.forward(&self.norm1.forward(x)?)?)?;
        let h = self
            .fc2
            .forward(&gelu(&self.fc1.forward(&self.norm2.forward(&x)?)?)?)?;
        x + h
    }
}

pub struct Encoder {
    backbone: Backbone,
    proj_w: Tensor,
    proj_b: Tensor,
    cls: Tensor,
    pos: Tensor,
    blocks: Vec<VitBlock>,
    norm: LayerNorm,
}

impl Encoder {
    pub fn new(vb: VarBuilder) -> Result<Self> {
        let blocks = (0..ENCODER_DEPTH)
            .map(|i| VitBlock::new(vb.pp(format!("blocks.{i}"))))
            .collect::<Result<_>>()?;
        Ok(Self {
            backbone: Backbone::new(vb.pp("patch_embed.backbone"))?,
            proj_w: vb.get((DIM, 1024, 1, 1), "patch_embed.proj.weight")?,
            proj_b: vb.get(DIM, "patch_embed.proj.bias")?,
            cls: vb.get((1, 1, DIM), "cls_token")?,
            pos: vb.get((1, 505, DIM), "pos_embed")?,
            blocks,
            norm: candle_nn::layer_norm(DIM, 1e-6, vb.pp("norm"))?,
        })
    }

    /// Encodes a picture (1 × 1 × H × W, both multiples of 32).
    pub fn forward(&self, img: &Tensor) -> Result<Tensor> {
        let (b, _, h, w) = img.dims4()?;
        let f = self.backbone.forward(img)?;
        let f = f
            .conv2d(&self.proj_w, 0, 1, 1, 1)?
            .broadcast_add(&self.proj_b.reshape((1, DIM, 1, 1))?)?;
        let x = f.flatten_from(2)?.transpose(1, 2)?; // B, N, DIM
        let (ph, pw) = (h / PATCH, w / PATCH);
        let row = MAX_WIDTH / PATCH;
        let mut idx: Vec<u32> = vec![0];
        for i in 0..ph {
            for j in 0..pw {
                idx.push((i * row + j + 1) as u32);
            }
        }
        let idx = Tensor::new(idx.as_slice(), img.device())?;
        let pos = self.pos.squeeze(0)?.index_select(&idx, 0)?.unsqueeze(0)?;
        let cls = self.cls.broadcast_as((b, 1, DIM))?;
        let mut x = Tensor::cat(&[&cls, &x], 1)?.broadcast_add(&pos)?;
        for blk in &self.blocks {
            x = blk.forward(&x)?;
        }
        self.norm.forward(&x)
    }
}

/// x-transformers attention (no biases on q, k, v; attention on
/// attention: the output is a GLU).
struct DecAttention {
    q: Linear,
    k: Linear,
    v: Linear,
    out: Linear,
}

impl DecAttention {
    fn new(vb: VarBuilder) -> Result<Self> {
        let inner = 64 * HEADS;
        Ok(Self {
            q: candle_nn::linear_no_bias(DIM, inner, vb.pp("to_q"))?,
            k: candle_nn::linear_no_bias(DIM, inner, vb.pp("to_k"))?,
            v: candle_nn::linear_no_bias(DIM, inner, vb.pp("to_v"))?,
            out: candle_nn::linear(inner, DIM * 2, vb.pp("to_out.0"))?,
        })
    }

    fn forward(&self, x: &Tensor, context: Option<&Tensor>, causal: bool) -> Result<Tensor> {
        let (b, n, _) = x.dims3()?;
        let ctx = context.unwrap_or(x);
        let m = ctx.dim(1)?;
        let split = |t: Tensor, len: usize| -> Result<Tensor> {
            t.reshape((b, len, HEADS, 64))?
                .transpose(1, 2)?
                .contiguous()
        };
        let q = split(self.q.forward(x)?, n)?;
        let k = split(self.k.forward(ctx)?, m)?;
        let v = split(self.v.forward(ctx)?, m)?;
        let mut dots = (q.matmul(&k.t()?)? * 0.125)?; // 64^-0.5
        if causal {
            let mask: Vec<f32> = (0..n)
                .flat_map(|i| (0..m).map(move |j| if j > i { f32::NEG_INFINITY } else { 0.0 }))
                .collect();
            let mask = Tensor::from_vec(mask, (n, m), x.device())?;
            dots = dots.broadcast_add(&mask)?;
        }
        let att = candle_nn::ops::softmax_last_dim(&dots)?;
        let out = att
            .matmul(&v)?
            .transpose(1, 2)?
            .reshape((b, n, 64 * HEADS))?;
        let out = self.out.forward(&out)?;
        // GLU: first half times sigmoid of the second.
        let a = out.narrow(D::Minus1, 0, DIM)?;
        let g = out.narrow(D::Minus1, DIM, DIM)?;
        a * candle_nn::ops::sigmoid(&g)?
    }
}

struct FeedForward {
    proj: Linear,
    out: Linear,
}

impl FeedForward {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let h = self.proj.forward(x)?;
        let inner = DIM * 4;
        let a = h.narrow(D::Minus1, 0, inner)?;
        let g = h.narrow(D::Minus1, inner, inner)?;
        self.out.forward(&(a * gelu(&g)?)?)
    }
}

enum Layer {
    SelfAttn(LayerNorm, DecAttention),
    CrossAttn(LayerNorm, DecAttention),
    Ff(LayerNorm, FeedForward),
}

pub struct Decoder {
    tokens: candle_nn::Embedding,
    pos: Tensor,
    layers: Vec<Layer>,
    norm: LayerNorm,
    logits: Linear,
}

impl Decoder {
    pub fn new(vb: VarBuilder) -> Result<Self> {
        let vb = vb.pp("net");
        let mut layers = Vec::new();
        for i in 0..DECODER_DEPTH * 3 {
            let l = vb.pp(format!("attn_layers.layers.{i}"));
            let norm = candle_nn::layer_norm(DIM, 1e-5, l.pp("0"))?;
            layers.push(match i % 3 {
                0 => Layer::SelfAttn(norm, DecAttention::new(l.pp("1"))?),
                1 => Layer::CrossAttn(norm, DecAttention::new(l.pp("1"))?),
                _ => Layer::Ff(
                    norm,
                    FeedForward {
                        proj: candle_nn::linear(DIM, DIM * 8, l.pp("1.net.0.proj"))?,
                        out: candle_nn::linear(DIM * 4, DIM, l.pp("1.net.2"))?,
                    },
                ),
            });
        }
        Ok(Self {
            tokens: candle_nn::embedding(8000, DIM, vb.pp("token_emb"))?,
            pos: vb.get((MAX_SEQ, DIM), "pos_emb.emb.weight")?,
            layers,
            norm: candle_nn::layer_norm(DIM, 1e-5, vb.pp("norm"))?,
            logits: candle_nn::linear(DIM, 8000, vb.pp("to_logits"))?,
        })
    }

    /// Logits for the next token after `tokens`.
    pub fn next(&self, tokens: &[u32], context: &Tensor) -> Result<Tensor> {
        let n = tokens.len();
        let ids = Tensor::new(tokens, context.device())?.unsqueeze(0)?;
        let mut x = self
            .tokens
            .forward(&ids)?
            .broadcast_add(&self.pos.narrow(0, 0, n)?.unsqueeze(0)?)?;
        for l in &self.layers {
            x = match l {
                Layer::SelfAttn(norm, a) => (&x + a.forward(&norm.forward(&x)?, None, true)?)?,
                Layer::CrossAttn(norm, a) => {
                    (&x + a.forward(&norm.forward(&x)?, Some(context), false)?)?
                }
                Layer::Ff(norm, f) => (&x + f.forward(&norm.forward(&x)?)?)?,
            };
        }
        let last = self.norm.forward(&x)?.narrow(1, n - 1, 1)?;
        self.logits.forward(&last)?.squeeze(0)?.squeeze(0)
    }
}

/// The whole model, loaded from `weights.pth`.
pub struct Model {
    pub encoder: Encoder,
    pub decoder: Decoder,
    pub device: Device,
}

impl Model {
    pub fn load(weights: &std::path::Path) -> Result<Self> {
        let device = Device::Cpu;
        let vb = VarBuilder::from_pth(weights, DType::F32, &device)?;
        Ok(Self {
            encoder: Encoder::new(vb.pp("encoder"))?,
            decoder: Decoder::new(vb.pp("decoder"))?,
            device,
        })
    }

    /// Reads tokens from a prepared picture (greedy decoding), with how
    /// sure the model was (mean log-probability of its choices).
    pub fn tokens(&self, img: &Tensor, limit: usize) -> Result<(Vec<u32>, f32)> {
        let ctx = self.encoder.forward(img)?;
        let mut out = vec![BOS];
        let mut sure = 0f32;
        let mut ended = false;
        for _ in 0..limit.min(MAX_SEQ - 1) {
            let logits = self.decoder.next(&out, &ctx)?;
            let logp = candle_nn::ops::log_softmax(&logits, 0)?;
            let next = logits.argmax(0)?.to_scalar::<u32>()?;
            sure += logp.get(next as usize)?.to_scalar::<f32>()?;
            if next == EOS {
                ended = true;
                break;
            }
            out.push(next);
        }
        let n = out.len() as f32;
        // Running out of room means the model lost its way.
        let sure = if ended { sure / n } else { f32::NEG_INFINITY };
        Ok((out[1..].to_vec(), sure))
    }
}
