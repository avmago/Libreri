import { useState } from "react";
import { FolderLock, KeyRound, Pencil, Plus, Trash2, UserPlus } from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { flattenFolders, useFolders } from "@/features/library";
import {
  COLOUR_NAMES,
  KIND_HINTS,
  KIND_LABELS,
  ProfileAvatar,
  colourOf,
  pinProblem,
  useCreateProfile,
  useDeleteProfile,
  useProfiles,
  useRecoveryCode,
  useSetAllowedFolders,
  useSetPin,
  useUpdateProfile,
} from "@/features/profiles";
import type { ProfileDto, ProfileKind, SessionDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Group, Row } from "../parts";

type Editing =
  | { kind: "new" }
  | { kind: "edit"; profile: ProfileDto }
  | { kind: "pin"; profile: ProfileDto }
  | { kind: "folders"; profile: ProfileDto }
  | null;

export function ProfileSettings({ session }: { session: SessionDto }) {
  const { data: profiles = [] } = useProfiles();
  const me = profiles.find((p) => p.id === session.profile.id) ?? session.profile;
  const remove = useDeleteProfile();
  const [editing, setEditing] = useState<Editing>(null);
  const owner = session.canManageProfiles;

  const onRemove = async (p: ProfileDto) => {
    const ok = await ask(
      `${p.name}'s reading status, positions, highlights and collections are removed. Their notes folder goes to the system Trash, so it can be restored.`,
      { title: `Remove ${p.name}?`, kind: "warning", okLabel: "Remove profile" },
    );
    if (ok) {
      remove.mutate(p.id, {
        onSuccess: () => toast.success(`Removed ${p.name}`),
        onError: (e) => toast.error("Could not remove the profile", { description: String(e) }),
      });
    }
  };

  return (
    <>
      <Group title="You" scope="yours">
        <div className="flex items-center gap-4 px-4 py-4">
          <ProfileAvatar name={me.name} colour={me.colour} guest={me.kind === "guest"} size={48} />
          <div className="flex min-w-0 flex-1 flex-col">
            <span className="text-[15px] font-semibold">{me.name}</span>
            <span className="text-muted-foreground">
              {KIND_LABELS[me.kind]} · {KIND_HINTS[me.kind]}
            </span>
          </div>
          {me.kind !== "guest" && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setEditing({ kind: "edit", profile: me })}
            >
              <Pencil /> Edit
            </Button>
          )}
        </div>
        {me.kind !== "guest" && (
          <Row
            label="PIN"
            help={
              me.hasPin
                ? "Your profile opens with a 6-digit PIN."
                : "Without a PIN, anyone using this computer can open your profile."
            }
          >
            <Button
              variant="outline"
              size="sm"
              onClick={() => setEditing({ kind: "pin", profile: me })}
            >
              <KeyRound /> {me.hasPin ? "Change PIN…" : "Set a PIN…"}
            </Button>
          </Row>
        )}
      </Group>

      <Group
        title="People who use this library"
        scope="library"
        description={
          owner
            ? "Everyone keeps their own reading status, highlights, notebooks and settings. PINs keep profiles private from casual use; they do not encrypt files."
            : "Only the library's owner can add or change profiles."
        }
      >
        {profiles.map((p) => (
          <div key={p.id} className="flex items-center gap-3 px-4 py-3">
            <ProfileAvatar name={p.name} colour={p.colour} guest={p.kind === "guest"} size={32} />
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="font-medium">
                {p.name}
                {p.id === me.id && (
                  <span className="font-normal text-muted-foreground"> (you)</span>
                )}
              </span>
              <span className="text-[12.5px] text-muted-foreground">
                {KIND_LABELS[p.kind]}
                {p.hasPin ? " · PIN" : ""}
                {p.kind === "kids"
                  ? ` · ${p.allowedFolders.length ? p.allowedFolders.join(", ") : "no folders yet"}`
                  : ""}
              </span>
            </div>
            {owner && p.id !== me.id && (
              <div className="flex gap-1">
                {p.kind === "kids" && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setEditing({ kind: "folders", profile: p })}
                  >
                    <FolderLock /> Folders
                  </Button>
                )}
                {p.kind !== "guest" && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setEditing({ kind: "pin", profile: p })}
                  >
                    <KeyRound /> {p.hasPin ? "Reset PIN" : "Set PIN"}
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Edit ${p.name}`}
                  onClick={() => setEditing({ kind: "edit", profile: p })}
                >
                  <Pencil />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove ${p.name}`}
                  onClick={() => void onRemove(p)}
                >
                  <Trash2 />
                </Button>
              </div>
            )}
          </div>
        ))}
        {owner && (
          <div className="flex gap-2 px-4 py-3">
            <Button size="sm" onClick={() => setEditing({ kind: "new" })}>
              <UserPlus /> Add a profile…
            </Button>
          </div>
        )}
      </Group>

      <ProfileDialog editing={editing} session={session} onClose={() => setEditing(null)} />
    </>
  );
}

function ProfileDialog({
  editing,
  session,
  onClose,
}: {
  editing: Editing;
  session: SessionDto;
  onClose: () => void;
}) {
  const title =
    editing?.kind === "new"
      ? "Add a profile"
      : editing?.kind === "edit"
        ? `Edit ${editing.profile.name}`
        : editing?.kind === "pin"
          ? editing.profile.id === session.profile.id
            ? editing.profile.hasPin
              ? "Change your PIN"
              : "Set a PIN"
            : `PIN for ${editing.profile.name}`
          : editing?.kind === "folders"
            ? `Folders ${editing.profile.name} can open`
            : "";
  return (
    <Dialog open={editing !== null} onOpenChange={(o) => !o && onClose()} title={title}>
      {editing?.kind === "new" && <ProfileForm onDone={onClose} />}
      {editing?.kind === "edit" && (
        <ProfileForm
          profile={editing.profile}
          canChangeKind={session.canManageProfiles && editing.profile.id !== session.profile.id}
          onDone={onClose}
        />
      )}
      {editing?.kind === "pin" && (
        <PinForm
          profile={editing.profile}
          isSelf={editing.profile.id === session.profile.id}
          onDone={onClose}
        />
      )}
      {editing?.kind === "folders" && <FoldersForm profile={editing.profile} onDone={onClose} />}
    </Dialog>
  );
}

const NEW_KINDS: ProfileKind[] = ["standard", "kids", "guest"];

function ProfileForm({
  profile,
  canChangeKind = true,
  onDone,
}: {
  profile?: ProfileDto;
  canChangeKind?: boolean;
  onDone: () => void;
}) {
  const create = useCreateProfile();
  const update = useUpdateProfile();
  const [name, setName] = useState(profile?.name ?? "");
  const [colour, setColour] = useState(profile?.colour ?? "blue");
  const [kind, setKind] = useState<ProfileKind>(profile?.kind ?? "standard");
  const [pin, setPin] = useState("");
  const error = create.error ?? update.error;
  const pinIssue = pin ? pinProblem(pin) : null;
  const kinds = profile
    ? profile.kind === "owner"
      ? ["owner" as const]
      : NEW_KINDS.filter((k) => k !== "guest" || profile.kind === "guest")
    : NEW_KINDS;

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        const done = { onSuccess: onDone };
        if (profile) update.mutate({ id: profile.id, name, colour, kind }, done);
        else create.mutate({ name, colour, kind, pin: pin || null }, done);
      }}
    >
      <div className="flex items-center gap-4">
        <ProfileAvatar name={name || "?"} colour={colour} guest={kind === "guest"} size={52} />
        <label className="flex flex-1 flex-col gap-1.5">
          <span className="font-medium">Name</span>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
            placeholder="Jane Smith"
            maxLength={40}
          />
        </label>
      </div>
      <fieldset className="flex flex-col gap-1.5">
        <legend className="pb-1.5 font-medium">Colour</legend>
        <div className="flex flex-wrap gap-2" role="radiogroup" aria-label="Colour">
          {COLOUR_NAMES.map((c) => (
            <button
              key={c}
              type="button"
              role="radio"
              aria-checked={colour === c}
              aria-label={c}
              onClick={() => setColour(c)}
              className={cn(
                "size-7 rounded-full",
                colour === c && "ring-2 ring-ring ring-offset-2 ring-offset-background",
              )}
              style={{ background: colourOf(c) }}
            />
          ))}
        </div>
      </fieldset>
      {canChangeKind && kinds.length > 1 && (
        <fieldset className="flex flex-col gap-2">
          <legend className="pb-1.5 font-medium">Type</legend>
          {kinds.map((k) => (
            <label
              key={k}
              className="flex items-start gap-2.5 rounded-md border p-2.5 has-[:checked]:border-primary"
            >
              <input
                type="radio"
                name="kind"
                checked={kind === k}
                onChange={() => setKind(k)}
                className="mt-0.5 accent-primary"
              />
              <span className="flex flex-col">
                <span className="font-medium">{KIND_LABELS[k]}</span>
                <span className="text-[12.5px] text-muted-foreground">{KIND_HINTS[k]}</span>
              </span>
            </label>
          ))}
        </fieldset>
      )}
      {!profile && kind !== "guest" && (
        <label className="flex flex-col gap-1.5">
          <span className="font-medium">PIN (optional)</span>
          <Input
            value={pin}
            onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 6))}
            inputMode="numeric"
            type="password"
            autoComplete="new-password"
            placeholder="6 digits"
            aria-invalid={!!pinIssue}
            className="w-32 font-mono tracking-widest"
          />
          {pinIssue && <span className="text-xs text-destructive">{pinIssue}</span>}
        </label>
      )}
      {error && <p className="text-destructive">{error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={!name.trim() || !!pinIssue || create.isPending || update.isPending}
        >
          {profile ? (
            "Save"
          ) : (
            <>
              <Plus /> Add profile
            </>
          )}
        </Button>
      </div>
    </form>
  );
}

function PinField({
  label,
  value,
  onChange,
  autoFocus,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  autoFocus?: boolean;
}) {
  return (
    <label className="flex items-center justify-between gap-4">
      <span className="font-medium">{label}</span>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value.replace(/\D/g, "").slice(0, 6))}
        inputMode="numeric"
        type="password"
        autoComplete="off"
        autoFocus={autoFocus}
        placeholder="••••••"
        className="w-32 font-mono tracking-widest"
      />
    </label>
  );
}

function PinForm({
  profile,
  isSelf,
  onDone,
}: {
  profile: ProfileDto;
  isSelf: boolean;
  onDone: () => void;
}) {
  const setPin = useSetPin();
  const showCode = useRecoveryCode((s) => s.show);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const needsCurrent = isSelf && profile.hasPin;
  const issue = next ? pinProblem(next) : null;
  const mismatch = again && next !== again ? "The two PINs are different." : null;

  const save = (newPin: string | null) =>
    setPin.mutate(
      { id: profile.id, currentPin: needsCurrent ? current : null, newPin },
      {
        onSuccess: (code) => {
          toast.success(newPin ? "PIN saved" : "PIN removed");
          onDone();
          if (code) showCode(code);
        },
      },
    );

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        save(next);
      }}
    >
      <p className="text-muted-foreground">
        {isSelf
          ? "Six digits. Avoid repeated digits and runs like 123456."
          : `${profile.name} will need this PIN to open their profile. Tell them what it is; their notes are kept.`}
      </p>
      {needsCurrent && (
        <PinField label="Current PIN" value={current} onChange={setCurrent} autoFocus />
      )}
      <PinField label="New PIN" value={next} onChange={setNext} autoFocus={!needsCurrent} />
      <PinField label="New PIN again" value={again} onChange={setAgain} />
      {(issue ?? mismatch) && <p className="text-[12.5px] text-destructive">{issue ?? mismatch}</p>}
      {setPin.error && <p className="text-destructive">{setPin.error.message}</p>}
      <div className="flex items-center justify-end gap-2 pt-1">
        {profile.hasPin && (
          <Button
            type="button"
            variant="ghost"
            className="mr-auto text-destructive"
            disabled={needsCurrent && current.length !== 6}
            onClick={() => save(null)}
          >
            Remove PIN
          </Button>
        )}
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={
            !!issue ||
            next.length !== 6 ||
            next !== again ||
            (needsCurrent && current.length !== 6) ||
            setPin.isPending
          }
        >
          Save PIN
        </Button>
      </div>
    </form>
  );
}

function FoldersForm({ profile, onDone }: { profile: ProfileDto; onDone: () => void }) {
  const { data: folders = [] } = useFolders();
  const save = useSetAllowedFolders();
  const [chosen, setChosen] = useState<string[]>(profile.allowedFolders);
  const flat = flattenFolders(folders);
  const covered = (path: string) => chosen.some((c) => path === c || path.startsWith(`${c}/`));
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate({ id: profile.id, folders: chosen }, { onSuccess: onDone });
      }}
    >
      <p className="text-muted-foreground">
        {profile.name} sees only the books in these folders and the folders inside them.
      </p>
      <div className="max-h-80 overflow-y-auto rounded-md border p-1">
        {flat.length === 0 && (
          <p className="p-3 text-muted-foreground">Create folders in the library first.</p>
        )}
        {flat.map(({ folder, depth }) => {
          const direct = chosen.includes(folder.path);
          const inherited = !direct && covered(folder.path);
          return (
            <label
              key={folder.path}
              className="flex h-8 items-center gap-2.5 rounded px-2 hover:bg-muted/60"
              style={{ paddingLeft: 8 + depth * 16 }}
            >
              <input
                type="checkbox"
                checked={direct || inherited}
                disabled={inherited}
                onChange={(e) =>
                  setChosen(
                    e.target.checked
                      ? [...chosen.filter((c) => !c.startsWith(`${folder.path}/`)), folder.path]
                      : chosen.filter((c) => c !== folder.path),
                  )
                }
                className="size-4 accent-primary"
              />
              <span className="flex-1 truncate">{folder.name}</span>
              <span className="text-[12px] text-muted-foreground">{folder.totalCount}</span>
            </label>
          );
        })}
      </div>
      {save.error && <p className="text-destructive">{save.error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={save.isPending}>
          Save
        </Button>
      </div>
    </form>
  );
}
