// SPDX-License-Identifier: MIT OR Apache-2.0
// The living style guide: every token and every primitive state, in the current theme.
// Dev builds only: the sidebar entry and this module are left out of release builds
// (import.meta.env.DEV), so it never ships.

import { type ReactNode, useState } from "react";
import {
  Badge,
  Button,
  Checkbox,
  Chip,
  Dialog,
  Dropdown,
  Icon,
  IconButton,
  type IconName,
  Notice,
  ProgressBar,
  Radio,
  Segmented,
  SidePanel,
  Spinner,
  Switch,
  useToast,
} from "../ui";

// Full class names, so Tailwind finds them when it scans the source.
const colorTokens = [
  { name: "ground", swatch: "bg-[var(--ground)]" },
  { name: "side", swatch: "bg-[var(--side)]" },
  { name: "card", swatch: "bg-[var(--card)]" },
  { name: "raised", swatch: "bg-[var(--raised)]" },
  { name: "line", swatch: "bg-[var(--line)]" },
  { name: "hover", swatch: "bg-[var(--hover)]" },
  { name: "text", swatch: "bg-[var(--text)]" },
  { name: "text2", swatch: "bg-[var(--text2)]" },
  { name: "muted", swatch: "bg-[var(--muted)]" },
  { name: "accent", swatch: "bg-[var(--accent)]" },
  { name: "btn", swatch: "bg-[var(--btn)]" },
  { name: "yellow", swatch: "bg-[var(--yellow)]" },
  { name: "ok", swatch: "bg-[var(--ok)]" },
  { name: "warn", swatch: "bg-[var(--warn)]" },
  { name: "bad", swatch: "bg-[var(--bad)]" },
  { name: "switch-off", swatch: "bg-[var(--switch-off)]" },
  { name: "pill", swatch: "bg-[var(--pill)]" },
  { name: "pill-ink", swatch: "bg-[var(--pill-ink)]" },
  { name: "scroll", swatch: "bg-[var(--scroll)]" },
  { name: "title-a", swatch: "bg-[var(--title-a)]" },
  { name: "title-b", swatch: "bg-[var(--title-b)]" },
];

const icons: IconName[] = [
  "dashboard",
  "download",
  "sliders",
  "layers",
  "bag",
  "bug",
  "book",
  "palette",
  "moon",
  "sun",
  "chevronDown",
  "chevronUp",
  "chevronLeft",
  "chevronRight",
  "check",
  "close",
  "pause",
  "play",
  "plus",
  "clock",
  "search",
  "warning",
  "shield",
  "heart",
  "eye",
  "folder",
  "refresh",
  "trash",
  "info",
];

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="card panel stack">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

export default function StyleGuide({
  theme,
  onTheme,
}: {
  theme: "dark" | "light";
  onTheme: (t: "dark" | "light") => void;
}) {
  const toast = useToast();
  const [on, setOn] = useState(true);
  const [checks, setChecks] = useState({ a: true, b: false });
  const [radio, setRadio] = useState("newest");
  const [category, setCategory] = useState("all");
  const [chars, setChars] = useState<string[]>(["anby"]);
  const [dialog, setDialog] = useState(false);
  const [panel, setPanel] = useState(false);

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Style guide</h1>
          <p>Tokens and primitives from docs/08, in the current theme. Dev builds only.</p>
        </div>
        <Segmented
          label="Theme"
          value={theme}
          onChange={onTheme}
          options={[
            { value: "dark", label: "Dark" },
            { value: "light", label: "Light" },
          ]}
        />
      </div>

      <div className="stack">
        <Section title="Color tokens">
          <div className="sg-swatches">
            {colorTokens.map((t) => (
              <div key={t.name} className="sg-swatch">
                <span className={t.swatch} />
                <span className="mono">--{t.name}</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Type">
          <div className="text-[30px] font-black tracking-[-0.4px]">Page title · 30 / 900</div>
          <div className="text-[18px] font-black">Section · 18 / 900</div>
          <div className="text-[15px] font-black">Card title · 15 / 900</div>
          <div>Body · 14 / 600. Mods only load when the game starts through XXMI.</div>
          <div className="sub">Caption · 12 / 700</div>
          <div className="kicker">Overline · 11 / 800, tracked</div>
          <div className="brand-title sg-brand">Gacha Mod Manager</div>
          <div className="mono">Mono · ZZMI\Mods\GMM\Casual Velina [gb-688491]</div>
        </Section>

        <Section title="Buttons">
          <div className="sg-row">
            <Button>Default</Button>
            <Button variant="primary" icon="download">
              Primary
            </Button>
            <Button variant="launch" icon="play">
              Launch
            </Button>
            <Button variant="danger" icon="trash">
              Danger
            </Button>
            <Button variant="link">Link</Button>
            <Button disabled>Disabled</Button>
            <Button variant="primary" disabled>
              Primary disabled
            </Button>
          </div>
          <div className="sg-row">
            <Button size="sm">Small</Button>
            <Button size="sm" variant="primary">
              Small primary
            </Button>
          </div>
        </Section>

        <Section title="Icon buttons">
          <div className="sg-row">
            <IconButton icon="pause" label="Pause" />
            <IconButton icon="close" label="Cancel" />
            <IconButton icon="download" label="Download" variant="primary" />
            <IconButton icon="refresh" label="Check again" size="sm" />
            <IconButton icon="close" label="Disabled" disabled />
            <div className="art a2 sg-art">
              <IconButton icon="sliders" label="Mod settings (on art)" variant="art" size="sm" />
            </div>
          </div>
        </Section>

        <Section title="Switch, checkbox, radio">
          <div className="sg-row">
            <Switch checked={on} onChange={setOn} label="Example" showState />
            <Switch checked={false} onChange={() => {}} label="Off example" showState />
            <Switch checked onChange={() => {}} label="Disabled example" disabled />
          </div>
          <div className="sg-row">
            <Checkbox checked={checks.a} onChange={(a) => setChecks((c) => ({ ...c, a }))}>
              Anby Weapon (Demon Sword)
            </Checkbox>
            <Checkbox checked={checks.b} onChange={(b) => setChecks((c) => ({ ...c, b }))}>
              Anby Weapon (Wind Sword)
            </Checkbox>
            <Checkbox checked disabled onChange={() => {}}>
              Installed (locked)
            </Checkbox>
          </div>
          <div className="opt-list max-w-[520px]">
            {[
              { v: "newest", t: "homewear_belle_v2.zip", s: "Newest · 27.3 MiB · 20 Oct 2026" },
              { v: "old", t: "homewear_belle.zip", s: "First upload · 26.8 MiB · 2 Sep 2026" },
            ].map((o) => (
              <Radio key={o.v} className="opt-row" name="file" checked={radio === o.v} onChange={() => setRadio(o.v)}>
                <span className="grow">
                  <b className="block font-extrabold">{o.t}</b>
                  <span className="sub">{o.s}</span>
                </span>
              </Radio>
            ))}
          </div>
        </Section>

        <Section title="Dropdowns">
          <div className="sg-row">
            <Dropdown
              label="Category"
              value={category}
              onChange={setCategory}
              options={[
                { value: "all", label: "All Mods" },
                { value: "skins", label: "Character Skins" },
                { value: "bangboo", label: "Bangboo Skins" },
                { value: "ui", label: "UI" },
              ]}
            />
            <Dropdown
              multiple
              label="Characters"
              allLabel="All characters"
              value={chars}
              onChange={setChars}
              options={[
                { value: "anby", label: "Anby" },
                { value: "belle", label: "Belle" },
                { value: "ellen", label: "Ellen" },
                { value: "miyabi", label: "Miyabi" },
              ]}
            />
            <Dropdown
              label="Characters"
              value="all"
              onChange={() => {}}
              options={[{ value: "all", label: "All characters" }]}
              disabled
              disabledReason="Pick one game to filter by character"
            />
          </div>
        </Section>

        <Section title="Chips and badges">
          <div className="sg-row">
            <Chip icon="download">5 mods installed</Chip>
            <Chip dot="ok">XXMI found</Chip>
            <Chip dot="warn">Needs setup</Chip>
            <Chip dot="bad">XXMI not found</Chip>
          </div>
          <div className="sg-row">
            <Badge tone="ok">Installed</Badge>
            <Badge tone="yellow">Trending today</Badge>
            <Badge tone="quiet">WIP</Badge>
            <Badge tone="warn">Folder missing</Badge>
            <Badge tone="bad">RAR</Badge>
            <span className="art a3 sg-art">
              <Badge tone="art">Anby</Badge>
              <Badge tone="game">ZZZ</Badge>
            </span>
            <span className="count-bubble" style={{ marginLeft: 0 }}>
              3
            </span>
            <span className="count-bubble is-wide" style={{ marginLeft: 0 }}>
              12
            </span>
          </div>
        </Section>

        <Section title="Notices">
          <Notice tone="warn" title="Phoenix_duplicate conflicts with Phoenix." actions={<Button>Keep it off</Button>}>
            Both replace the same 8 things in game; only one shows.
          </Notice>
          <Notice tone="bad" title="Can't install: RAR archives aren't supported yet.">
            Nothing was downloaded.
          </Notice>
          <Notice tone="ok" title="Installed.">
            Casual Velina is on.
          </Notice>
          <Notice tone="info" title="Fetching the archive to look inside.">
            GMM installs nothing until you confirm.
          </Notice>
        </Section>

        <Section title="Progress">
          <div className="stack max-w-[520px]">
            <span className="sub">Downloading · 62% · 16.9 of 27.3 MiB</span>
            <ProgressBar value={0.62} label="Downloading" />
            <span className="sub">Extracting and scanning</span>
            <ProgressBar label="Extracting" />
            <span className="flex items-center gap-2 text-accent">
              <Spinner /> Checking for updates
            </span>
          </div>
        </Section>

        <Section title="Overlays">
          <div className="sg-row">
            <Button onClick={() => setDialog(true)}>Open dialog</Button>
            <Button onClick={() => setPanel(true)}>Open side panel</Button>
            <Button onClick={() => toast("info", "Fetching homewear_belle_v2.zip into the download cache.")}>
              Info toast
            </Button>
            <Button onClick={() => toast("ok", "Casual Velina installed and switched on.")}>Success toast</Button>
            <Button onClick={() => toast("bad", "GameBanana didn't answer. Try again in a minute.")}>
              Error toast
            </Button>
          </div>
        </Section>

        <Section title="Icons">
          <div className="sg-row">
            {icons.map((name) => (
              <span key={name} title={name} className="icon-btn sg-icon">
                <Icon name={name} />
              </span>
            ))}
          </div>
        </Section>
      </div>

      {dialog && (
        <Dialog
          eyebrow="Install"
          title="Fixed Yae Sakura Sword Pack"
          media={<div className="thumb art a2 sg-thumb" />}
          onClose={() => setDialog(false)}
          footer={
            <>
              <Button onClick={() => setDialog(false)}>Cancel</Button>
              <Button variant="primary" onClick={() => setDialog(false)}>
                Install 2 versions
              </Button>
            </>
          }
        >
          <div className="kv">
            <div className="kv-row">
              <span>File</span>
              <b>anby_fixed_yae_sakura_swords_.zip · 6.0 MiB</b>
            </div>
            <div className="kv-row">
              <span>Installs to</span>
              <b>Zenless Zone Zero › ZZMI\Mods\GMM</b>
            </div>
          </div>
          <p className="hint">Versions that replace the same thing can't be on together.</p>
          <Checkbox checked onChange={() => {}}>
            Switch it on after installing
          </Checkbox>
        </Dialog>
      )}
      {panel && (
        <SidePanel eyebrow="Mod settings" title="Casual Velina" onClose={() => setPanel(false)}>
          <section className="drawer-sec">
            <div className="flex items-center gap-3">
              <b className="grow">Switched on</b>
              <Switch checked={on} onChange={setOn} label="Casual Velina" showState />
            </div>
          </section>
          <section className="drawer-sec">
            <h3 className="sec-h">Details</h3>
            <div className="kv">
              <div className="kv-row">
                <span>Installed</span>
                <b>24 Oct 2026</b>
              </div>
              <div className="kv-row">
                <span>Size</span>
                <b>131.0 MiB</b>
              </div>
            </div>
          </section>
          <section className="drawer-sec">
            <Button variant="danger" block icon="trash">
              Uninstall
            </Button>
          </section>
        </SidePanel>
      )}
    </>
  );
}
