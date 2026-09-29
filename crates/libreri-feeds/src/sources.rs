//! arXiv's categories and a short list of suggested sources.

use serde::Serialize;

/// A group of arXiv categories ("Computer Science").
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArxivGroup {
    pub name: &'static str,
    pub categories: Vec<ArxivCategory>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArxivCategory {
    /// "cs.AI"
    pub code: &'static str,
    /// "Artificial Intelligence"
    pub name: &'static str,
}

type Table = &'static [(&'static str, &'static [(&'static str, &'static str)])];

/// arXiv's category taxonomy (arxiv.org/category_taxonomy).
const ARXIV: Table = &[
    (
        "Computer Science",
        &[
            ("cs.AI", "Artificial Intelligence"),
            ("cs.AR", "Hardware Architecture"),
            ("cs.CC", "Computational Complexity"),
            ("cs.CE", "Computational Engineering, Finance, and Science"),
            ("cs.CG", "Computational Geometry"),
            ("cs.CL", "Computation and Language"),
            ("cs.CR", "Cryptography and Security"),
            ("cs.CV", "Computer Vision and Pattern Recognition"),
            ("cs.CY", "Computers and Society"),
            ("cs.DB", "Databases"),
            ("cs.DC", "Distributed, Parallel, and Cluster Computing"),
            ("cs.DL", "Digital Libraries"),
            ("cs.DM", "Discrete Mathematics"),
            ("cs.DS", "Data Structures and Algorithms"),
            ("cs.ET", "Emerging Technologies"),
            ("cs.FL", "Formal Languages and Automata Theory"),
            ("cs.GL", "General Literature"),
            ("cs.GR", "Graphics"),
            ("cs.GT", "Computer Science and Game Theory"),
            ("cs.HC", "Human-Computer Interaction"),
            ("cs.IR", "Information Retrieval"),
            ("cs.IT", "Information Theory"),
            ("cs.LG", "Machine Learning"),
            ("cs.LO", "Logic in Computer Science"),
            ("cs.MA", "Multiagent Systems"),
            ("cs.MM", "Multimedia"),
            ("cs.MS", "Mathematical Software"),
            ("cs.NA", "Numerical Analysis"),
            ("cs.NE", "Neural and Evolutionary Computing"),
            ("cs.NI", "Networking and Internet Architecture"),
            ("cs.OH", "Other Computer Science"),
            ("cs.OS", "Operating Systems"),
            ("cs.PF", "Performance"),
            ("cs.PL", "Programming Languages"),
            ("cs.RO", "Robotics"),
            ("cs.SC", "Symbolic Computation"),
            ("cs.SD", "Sound"),
            ("cs.SE", "Software Engineering"),
            ("cs.SI", "Social and Information Networks"),
            ("cs.SY", "Systems and Control"),
        ],
    ),
    (
        "Economics",
        &[
            ("econ.EM", "Econometrics"),
            ("econ.GN", "General Economics"),
            ("econ.TH", "Theoretical Economics"),
        ],
    ),
    (
        "Electrical Engineering and Systems Science",
        &[
            ("eess.AS", "Audio and Speech Processing"),
            ("eess.IV", "Image and Video Processing"),
            ("eess.SP", "Signal Processing"),
            ("eess.SY", "Systems and Control"),
        ],
    ),
    (
        "Mathematics",
        &[
            ("math.AC", "Commutative Algebra"),
            ("math.AG", "Algebraic Geometry"),
            ("math.AP", "Analysis of PDEs"),
            ("math.AT", "Algebraic Topology"),
            ("math.CA", "Classical Analysis and ODEs"),
            ("math.CO", "Combinatorics"),
            ("math.CT", "Category Theory"),
            ("math.CV", "Complex Variables"),
            ("math.DG", "Differential Geometry"),
            ("math.DS", "Dynamical Systems"),
            ("math.FA", "Functional Analysis"),
            ("math.GM", "General Mathematics"),
            ("math.GN", "General Topology"),
            ("math.GR", "Group Theory"),
            ("math.GT", "Geometric Topology"),
            ("math.HO", "History and Overview"),
            ("math.IT", "Information Theory"),
            ("math.KT", "K-Theory and Homology"),
            ("math.LO", "Logic"),
            ("math.MG", "Metric Geometry"),
            ("math.MP", "Mathematical Physics"),
            ("math.NA", "Numerical Analysis"),
            ("math.NT", "Number Theory"),
            ("math.OA", "Operator Algebras"),
            ("math.OC", "Optimization and Control"),
            ("math.PR", "Probability"),
            ("math.QA", "Quantum Algebra"),
            ("math.RA", "Rings and Algebras"),
            ("math.RT", "Representation Theory"),
            ("math.SG", "Symplectic Geometry"),
            ("math.SP", "Spectral Theory"),
            ("math.ST", "Statistics Theory"),
        ],
    ),
    (
        "Astrophysics",
        &[
            ("astro-ph.CO", "Cosmology and Nongalactic Astrophysics"),
            ("astro-ph.EP", "Earth and Planetary Astrophysics"),
            ("astro-ph.GA", "Astrophysics of Galaxies"),
            ("astro-ph.HE", "High Energy Astrophysical Phenomena"),
            (
                "astro-ph.IM",
                "Instrumentation and Methods for Astrophysics",
            ),
            ("astro-ph.SR", "Solar and Stellar Astrophysics"),
        ],
    ),
    (
        "Condensed Matter",
        &[
            ("cond-mat.dis-nn", "Disordered Systems and Neural Networks"),
            ("cond-mat.mes-hall", "Mesoscale and Nanoscale Physics"),
            ("cond-mat.mtrl-sci", "Materials Science"),
            ("cond-mat.other", "Other Condensed Matter"),
            ("cond-mat.quant-gas", "Quantum Gases"),
            ("cond-mat.soft", "Soft Condensed Matter"),
            ("cond-mat.stat-mech", "Statistical Mechanics"),
            ("cond-mat.str-el", "Strongly Correlated Electrons"),
            ("cond-mat.supr-con", "Superconductivity"),
        ],
    ),
    (
        "High Energy Physics and Gravitation",
        &[
            ("gr-qc", "General Relativity and Quantum Cosmology"),
            ("hep-ex", "High Energy Physics – Experiment"),
            ("hep-lat", "High Energy Physics – Lattice"),
            ("hep-ph", "High Energy Physics – Phenomenology"),
            ("hep-th", "High Energy Physics – Theory"),
        ],
    ),
    (
        "Mathematical Physics and Quantum Physics",
        &[
            ("math-ph", "Mathematical Physics"),
            ("quant-ph", "Quantum Physics"),
        ],
    ),
    (
        "Nonlinear Sciences",
        &[
            ("nlin.AO", "Adaptation and Self-Organizing Systems"),
            ("nlin.CD", "Chaotic Dynamics"),
            ("nlin.CG", "Cellular Automata and Lattice Gases"),
            ("nlin.PS", "Pattern Formation and Solitons"),
            ("nlin.SI", "Exactly Solvable and Integrable Systems"),
        ],
    ),
    (
        "Nuclear Physics",
        &[
            ("nucl-ex", "Nuclear Experiment"),
            ("nucl-th", "Nuclear Theory"),
        ],
    ),
    (
        "Physics",
        &[
            ("physics.acc-ph", "Accelerator Physics"),
            ("physics.ao-ph", "Atmospheric and Oceanic Physics"),
            ("physics.app-ph", "Applied Physics"),
            ("physics.atm-clus", "Atomic and Molecular Clusters"),
            ("physics.atom-ph", "Atomic Physics"),
            ("physics.bio-ph", "Biological Physics"),
            ("physics.chem-ph", "Chemical Physics"),
            ("physics.class-ph", "Classical Physics"),
            ("physics.comp-ph", "Computational Physics"),
            (
                "physics.data-an",
                "Data Analysis, Statistics and Probability",
            ),
            ("physics.ed-ph", "Physics Education"),
            ("physics.flu-dyn", "Fluid Dynamics"),
            ("physics.gen-ph", "General Physics"),
            ("physics.geo-ph", "Geophysics"),
            ("physics.hist-ph", "History and Philosophy of Physics"),
            ("physics.ins-det", "Instrumentation and Detectors"),
            ("physics.med-ph", "Medical Physics"),
            ("physics.optics", "Optics"),
            ("physics.plasm-ph", "Plasma Physics"),
            ("physics.pop-ph", "Popular Physics"),
            ("physics.soc-ph", "Physics and Society"),
            ("physics.space-ph", "Space Physics"),
        ],
    ),
    (
        "Quantitative Biology",
        &[
            ("q-bio.BM", "Biomolecules"),
            ("q-bio.CB", "Cell Behavior"),
            ("q-bio.GN", "Genomics"),
            ("q-bio.MN", "Molecular Networks"),
            ("q-bio.NC", "Neurons and Cognition"),
            ("q-bio.OT", "Other Quantitative Biology"),
            ("q-bio.PE", "Populations and Evolution"),
            ("q-bio.QM", "Quantitative Methods"),
            ("q-bio.SC", "Subcellular Processes"),
            ("q-bio.TO", "Tissues and Organs"),
        ],
    ),
    (
        "Quantitative Finance",
        &[
            ("q-fin.CP", "Computational Finance"),
            ("q-fin.EC", "Economics"),
            ("q-fin.GN", "General Finance"),
            ("q-fin.MF", "Mathematical Finance"),
            ("q-fin.PM", "Portfolio Management"),
            ("q-fin.PR", "Pricing of Securities"),
            ("q-fin.RM", "Risk Management"),
            ("q-fin.ST", "Statistical Finance"),
            ("q-fin.TR", "Trading and Market Microstructure"),
        ],
    ),
    (
        "Statistics",
        &[
            ("stat.AP", "Applications"),
            ("stat.CO", "Computation"),
            ("stat.ME", "Methodology"),
            ("stat.ML", "Machine Learning"),
            ("stat.OT", "Other Statistics"),
            ("stat.TH", "Statistics Theory"),
        ],
    ),
];

pub fn arxiv_groups() -> Vec<ArxivGroup> {
    ARXIV
        .iter()
        .map(|(name, cats)| ArxivGroup {
            name,
            categories: cats
                .iter()
                .map(|(code, name)| ArxivCategory { code, name })
                .collect(),
        })
        .collect()
}

/// A category's group and name.
pub fn arxiv_category(code: &str) -> Option<(&'static str, &'static str)> {
    ARXIV.iter().find_map(|(group, cats)| {
        cats.iter()
            .find(|(c, _)| c.eq_ignore_ascii_case(code))
            .map(|(_, name)| (*group, *name))
    })
}

/// The feed of new papers in an arXiv category.
pub fn arxiv_feed_url(code: &str) -> String {
    format!("https://rss.arxiv.org/rss/{code}")
}

/// A feed of the newest papers matching a search. Plain words must all
/// appear; arXiv's own syntax (`au:`, `ti:`, `cat:`, AND/OR) is kept.
pub fn arxiv_search_url(query: &str) -> Result<String, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("type what to search for".into());
    }
    let advanced = q.contains(':') || q.contains(" AND ") || q.contains(" OR ");
    let search = if advanced {
        q.to_owned()
    } else {
        q.split_whitespace()
            .map(|w| format!("all:{w}"))
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let mut u = url::Url::parse("https://export.arxiv.org/api/query").map_err(|e| e.to_string())?;
    u.query_pairs_mut()
        .append_pair("search_query", &search)
        .append_pair("sortBy", "submittedDate")
        .append_pair("sortOrder", "descending")
        .append_pair("max_results", "50");
    Ok(u.to_string())
}

/// A source offered in "Suggested".
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggested {
    pub title: &'static str,
    pub url: &'static str,
    /// "Preprints", "Journals", "Science news", …
    pub group: &'static str,
    pub about: &'static str,
}

pub fn suggested() -> Vec<Suggested> {
    let s = |group, title, url, about| Suggested {
        title,
        url,
        group,
        about,
    };
    vec![
        s(
            "Journals",
            "Nature",
            "https://www.nature.com/nature.rss",
            "Research and news from Nature",
        ),
        s(
            "Journals",
            "PLOS ONE",
            "https://journals.plos.org/plosone/feed/atom",
            "Open-access research in all fields",
        ),
        s(
            "Journals",
            "PLOS Biology",
            "https://journals.plos.org/plosbiology/feed/atom",
            "Open-access biology",
        ),
        s(
            "Journals",
            "eLife",
            "https://elifesciences.org/rss/recent.xml",
            "Life sciences and medicine",
        ),
        s(
            "Journals",
            "Journal of Open Source Software",
            "https://joss.theoj.org/papers/published.atom",
            "Papers about research software",
        ),
        s(
            "Science news",
            "Quanta Magazine",
            "https://www.quantamagazine.org/feed/",
            "Mathematics, physics, biology and computer science",
        ),
        s(
            "Science news",
            "IEEE Spectrum",
            "https://spectrum.ieee.org/feeds/feed.rss",
            "Engineering and technology",
        ),
        s(
            "Science news",
            "MIT Technology Review",
            "https://www.technologyreview.com/feed/",
            "Technology and its effects",
        ),
        s(
            "Tech",
            "Hacker News",
            "https://news.ycombinator.com/rss",
            "Links shared on Hacker News",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arxiv_names_and_addresses() {
        assert_eq!(
            arxiv_category("cs.lg"),
            Some(("Computer Science", "Machine Learning"))
        );
        assert_eq!(
            arxiv_feed_url("math.PR"),
            "https://rss.arxiv.org/rss/math.PR"
        );
        let u = arxiv_search_url("diffusion models").unwrap();
        assert!(u.contains("search_query=all%3Adiffusion+AND+all%3Amodels"));
        let u = arxiv_search_url("au:Tao AND cat:math.NT").unwrap();
        assert!(u.contains("search_query=au%3ATao+AND+cat%3Amath.NT"));
        // Every code is unique.
        let mut codes: Vec<_> = arxiv_groups()
            .into_iter()
            .flat_map(|g| g.categories.into_iter().map(|c| c.code))
            .collect();
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), n);
    }
}
