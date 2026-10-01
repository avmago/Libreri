import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Source, SourceInfo } from "@/lib/ipc";
import { useOnlineSettings, useSetOnlineSettings, useSetSourceKey } from "@/features/details";
import { useIndexStatus, useSetIndexKey } from "@/features/podcasts";
import { usePermissions } from "@/features/profiles";
import { Group, Row, Switch } from "../parts";

/** What each source is good for, shown under its name. */
const ABOUT: Record<Source, string> = {
  openLibrary: "Books by ISBN or title, with covers and subjects. Free.",
  googleBooks: "Books by ISBN or title, with descriptions and categories. Free.",
  crossref: "Papers, chapters and books with a DOI. Free.",
  openAlex: "Research papers, with topics that become categories. Free.",
  semanticScholar: "Papers by DOI or arXiv id, with fields of study. Free.",
  arxiv: "Preprints by arXiv id. Free.",
  comicVine: "Comics and manga. Needs your own free API key from comicvine.gamespot.com.",
  isbndb: "A large paid ISBN database. Needs your own ISBNdb API key.",
};

function KeyRow({ info }: { info: SourceInfo }) {
  const setKey = useSetSourceKey();
  const [key, setKeyText] = useState("");
  const save = (value: string | null) =>
    setKey.mutate(
      { source: info.source, key: value },
      {
        onSuccess: () => {
          setKeyText("");
          toast.success(value ? `${info.name} key saved` : `${info.name} key removed`);
        },
        onError: (e) => toast.error(e.message),
      },
    );
  return (
    <div className="flex items-center gap-2 px-4 pb-3">
      <Input
        type="password"
        autoComplete="off"
        spellCheck={false}
        aria-label={`${info.name} API key`}
        placeholder={info.keyHint ? `Saved key ${info.keyHint}` : "Paste your API key"}
        value={key}
        onChange={(e) => setKeyText(e.target.value)}
        className="font-mono"
      />
      <Button size="sm" disabled={!key.trim() || setKey.isPending} onClick={() => save(key)}>
        Save key
      </Button>
      {info.keyHint && (
        <Button size="sm" variant="ghost" onClick={() => save(null)}>
          Remove
        </Button>
      )}
    </div>
  );
}

/** Settings › Online details: sources, keys, filling details on import, podcasts. */
export function OnlineDetailsSettings() {
  const { editLibrary, keepsData } = usePermissions();
  return (
    <>
      {editLibrary ? (
        <BookSources />
      ) : (
        <p className="text-muted-foreground">
          Only profiles that can change the library can look up book details online.
        </p>
      )}
      {keepsData && <PodcastIndex />}
    </>
  );
}

/** Podcast Index: an open directory, searched with the user's own free key. */
function PodcastIndex() {
  const { data } = useIndexStatus();
  const setKey = useSetIndexKey();
  const [key, setKeyText] = useState("");
  const [secret, setSecret] = useState("");
  const save = (k: string, sec: string) =>
    setKey.mutate(
      { key: k.trim(), secret: sec.trim() },
      {
        onSuccess: () => {
          setKeyText("");
          setSecret("");
          toast.success(k ? "Podcast Index key saved" : "Podcast Index key removed");
        },
        onError: (e) => toast.error("The key was not saved", { description: e.message }),
      },
    );
  return (
    <Group
      title="Podcasts"
      scope="computer"
      description="Find podcasts searches Apple Podcasts, which needs no key. With your own free Podcast Index key you can also search its open directory and see popular shows. Only your search words are sent. The key stays on this computer and is never exported."
    >
      <Row
        label="Podcast Index"
        help={
          data?.configured
            ? `A key ending ${data.keyHint ?? ""} is saved.`
            : "Sign up free at podcastindex.org (api.podcastindex.org), then paste the API key and secret it emails you."
        }
      >
        {data?.configured ? (
          <Button
            size="sm"
            variant="ghost"
            disabled={setKey.isPending}
            onClick={() => save("", "")}
          >
            Remove
          </Button>
        ) : (
          <span className="text-[12px] text-muted-foreground">Not set</span>
        )}
      </Row>
      <form
        className="flex flex-wrap items-center gap-2 px-4 pb-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (key.trim() && secret.trim()) save(key, secret);
        }}
      >
        <Input
          autoComplete="off"
          spellCheck={false}
          aria-label="Podcast Index API key"
          placeholder={data?.configured ? "New API key" : "API key"}
          value={key}
          onChange={(e) => setKeyText(e.target.value)}
          className="min-w-40 flex-1 font-mono"
        />
        <Input
          type="password"
          autoComplete="off"
          spellCheck={false}
          aria-label="Podcast Index API secret"
          placeholder="API secret"
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          className="min-w-40 flex-1 font-mono"
        />
        <Button
          type="submit"
          size="sm"
          disabled={!key.trim() || !secret.trim() || setKey.isPending}
        >
          {setKey.isPending ? "Checking…" : "Check and save"}
        </Button>
      </form>
    </Group>
  );
}

function BookSources() {
  const { data } = useOnlineSettings();
  const change = useSetOnlineSettings();
  if (!data) return null;
  const enabled = data.sources.filter((s) => s.enabled).map((s) => s.source);
  const toggle = (s: SourceInfo, on: boolean) =>
    change.mutate({
      enabled: on ? [...enabled, s.source] : enabled.filter((x) => x !== s.source),
      fillOnImport: null,
    });

  return (
    <>
      <Group
        title="Sources"
        scope="computer"
        description="Where Find details online looks. Libreri sends only the ISBN, DOI, arXiv id, title and author of the book being looked up; never your notes, files or name."
      >
        {data.sources.map((s) => (
          <div key={s.source}>
            <Row label={s.name} help={ABOUT[s.source]}>
              <Switch
                label={`Use ${s.name}`}
                checked={s.enabled}
                onChange={(on) => {
                  if (on && s.needsKey && !s.keyHint) {
                    toast("Paste a key below to use " + s.name);
                    return;
                  }
                  toggle(s, on);
                }}
              />
            </Row>
            {s.needsKey && <KeyRow info={s} />}
          </div>
        ))}
      </Group>
      <Group
        title="New books"
        scope="computer"
        description="These settings and API keys stay on this computer. They are never exported with the library."
      >
        <Row
          label="Fill in missing details when importing"
          help="After an import, look each new book up and fill only the empty details and a missing cover, when the match is sure. Off by default because it sends titles to the sources."
        >
          <Switch
            label="Fill in missing details when importing"
            checked={data.fillOnImport}
            onChange={(on) => change.mutate({ enabled: null, fillOnImport: on })}
          />
        </Row>
      </Group>
    </>
  );
}
