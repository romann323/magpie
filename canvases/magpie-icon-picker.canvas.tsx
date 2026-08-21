import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Grid,
  H1,
  H2,
  Pill,
  Row,
  Stack,
  Text,
  useCanvasAction,
  useCanvasState,
  useHostTheme,
} from "cursor/canvas";

type IconOption = {
  id: number;
  name: string;
  vibe: string;
  file: string;
  svg: JSX.Element;
};

function IconFrame({ children }: { children: JSX.Element }) {
  return (
    <svg viewBox="0 0 128 128" width="96" height="96" aria-hidden="true">
      {children}
    </svg>
  );
}

const icons: IconOption[] = [
  {
    id: 1,
    name: "Profile",
    vibe: "Classic magpie silhouette — collector bird in profile with a bright find.",
    file: "design/icon-options/01-profile.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#4F46E5" />
        <path fill="#0F172A" d="M28 92c8-28 28-44 52-44 10 0 20 3 28 9l-6 14c-6-4-13-6-20-6-18 0-32 12-38 27H28z" />
        <path fill="#F8FAFC" d="M52 48c18 0 34 14 38 32 2 8 1 16-2 22l-8-4c2-5 3-11 2-17-3-12-13-20-26-20-8 0-15 3-20 8l-6-12c8-6 18-9 22-9z" />
        <path fill="#0F172A" d="M78 38c6 0 12 2 16 6l-10 8c-2-2-5-3-8-3-6 0-11 4-13 10l-6-3c3-10 12-18 21-18z" />
        <circle cx="86" cy="44" r="3.5" fill="#F8FAFC" />
        <path fill="#38BDF8" d="M98 52l14 6-14 4 4-10z" />
      </IconFrame>
    ),
  },
  {
    id: 2,
    name: "Monogram",
    vibe: "Bold M with a wing accent — minimal, works tiny in the taskbar.",
    file: "design/icon-options/02-monogram.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#312E81" />
        <path fill="#F8FAFC" d="M36 88V40h16l12 28 12-28h16v48h-14V62l-11 26h-12L43 62v26H36z" />
        <path fill="#A5B4FC" d="M94 36c12 8 18 20 18 32 0 4-1 8-2 12l-10-4c1-3 2-6 2-8 0-8-4-16-10-22l8-10z" />
      </IconFrame>
    ),
  },
  {
    id: 3,
    name: "Gem",
    vibe: "Magpie with a golden treasure — playful 'collect the good stuff' story.",
    file: "design/icon-options/03-gem.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#0E7490" />
        <ellipse cx="64" cy="96" rx="36" ry="8" fill="#083344" opacity="0.35" />
        <path fill="#0F172A" d="M34 78c6-22 22-36 42-36 8 0 16 2 22 6l-4 10c-5-3-11-4-17-4-14 0-26 10-31 24H34z" />
        <path fill="#F8FAFC" d="M48 46c16 0 28 12 32 28 1 5 0 10-2 14l-6-3c1-3 2-7 1-10-2-10-10-18-21-18-6 0-12 2-16 6l-4-8c6-5 14-9 16-9z" />
        <path fill="#0F172A" d="M72 40c5 0 9 2 12 5l-7 6c-1-2-3-3-5-3-4 0-7 3-8 7l-5-2c2-7 7-13 13-13z" />
        <circle cx="79" cy="44" r="2.5" fill="#F8FAFC" />
        <path fill="#FDE047" d="M88 58l10 8-12 4 2-12z" />
        <path fill="#FACC15" d="M92 62l6 5-7 2 1-7z" />
      </IconFrame>
    ),
  },
  {
    id: 4,
    name: "Nest",
    vibe: "Bird guarding a 2×2 grid — library organizer, metadata facets.",
    file: "design/icon-options/04-nest.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#4338CA" />
        <rect x="28" y="68" width="32" height="32" rx="6" fill="#6366F1" />
        <rect x="68" y="68" width="32" height="32" rx="6" fill="#818CF8" />
        <rect x="28" y="28" width="32" height="32" rx="6" fill="#818CF8" />
        <rect x="68" y="28" width="32" height="32" rx="6" fill="#6366F1" />
        <path fill="#0F172A" d="M46 26c10-8 22-8 32 0 6 5 10 12 10 20H36c0-8 4-15 10-20z" />
        <path fill="#F8FAFC" d="M52 18c8-4 16-4 24 0 4 2 7 6 8 10H44c1-4 4-8 8-10z" />
        <circle cx="72" cy="22" r="2" fill="#0F172A" />
        <path fill="#FDE047" d="M82 24l4 4-5 2 1-6z" />
      </IconFrame>
    ),
  },
  {
    id: 5,
    name: "Medallion",
    vibe: "Circular badge — premium, calm, reads well as a macOS dock icon.",
    file: "design/icon-options/05-medallion.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#1E1B4B" />
        <circle cx="64" cy="64" r="44" fill="#4F46E5" />
        <circle cx="64" cy="64" r="36" fill="#312E81" />
        <path fill="#F8FAFC" d="M64 34c14 0 26 10 30 24 2 6 2 12 0 18-4 14-16 24-30 24s-26-10-30-24c-2-6-2-12 0-18 4-14 16-24 30-24zm0 8c-10 0-18 8-20 18-1 4-1 8 0 12 2 10 10 18 20 18s18-8 20-18c1-4 1-8 0-12-2-10-10-18-20-18z" />
        <path fill="#0F172A" d="M64 42c8 0 14 6 16 14 1 3 1 6 0 9-2 8-8 14-16 14s-14-6-16-14c-1-3-1-6 0-9 2-8 8-14 16-14z" />
        <circle cx="70" cy="50" r="2.5" fill="#F8FAFC" />
        <path fill="#A5B4FC" d="M78 56l8 4-8 3 2-7z" />
      </IconFrame>
    ),
  },
  {
    id: 6,
    name: "Origami",
    vibe: "Geometric folded bird — sharp, modern, slightly technical.",
    file: "design/icon-options/06-origami.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#0369A1" />
        <path fill="#0F172A" d="M64 24 96 88H32L64 24z" />
        <path fill="#F8FAFC" d="M64 24 80 88H48L64 24z" />
        <path fill="#38BDF8" d="M64 24 96 88 64 72 32 88 64 24z" />
        <path fill="#0F172A" d="M58 52h12l-6 14-6-14z" />
        <circle cx="64" cy="46" r="3" fill="#F8FAFC" />
      </IconFrame>
    ),
  },
  {
    id: 7,
    name: "Tagged",
    vibe: "Bird plus metadata tag — direct nod to ratings, tags, and search.",
    file: "design/icon-options/07-tagged.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#5B21B6" />
        <path fill="#EDE9FE" d="M78 88 52 62l22-22 26 26-22 22z" />
        <circle cx="68" cy="48" r="6" fill="#7C3AED" />
        <path fill="#0F172A" d="M30 74c4-18 18-30 36-30 6 0 12 1 17 4l-3 8c-4-2-8-3-13-3-12 0-22 8-26 21H30z" />
        <path fill="#F8FAFC" d="M42 44c12 0 22 8 26 20 1 4 1 8 0 11l-5-2c1-2 1-5 0-7-2-8-9-14-18-14-5 0-9 2-12 5l-3-7c5-4 11-6 12-6z" />
        <circle cx="62" cy="48" r="2" fill="#0F172A" />
      </IconFrame>
    ),
  },
  {
    id: 8,
    name: "Perch",
    vibe: "Bird perched on stacked layers — files from many folders and drives.",
    file: "design/icon-options/08-perch.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#334155" />
        <rect x="24" y="78" width="80" height="12" rx="4" fill="#64748B" />
        <rect x="30" y="62" width="68" height="10" rx="3" fill="#475569" />
        <rect x="36" y="48" width="56" height="8" rx="3" fill="#64748B" />
        <path fill="#0F172A" d="M48 48c8-16 24-24 40-16 4 2 7 5 9 9l-6 6c-1-2-3-4-6-5-10-4-22 2-28 12l-9-6z" />
        <path fill="#F8FAFC" d="M56 32c12 0 22 8 24 20 0 3 0 6-1 8l-5-2c1-2 1-4 0-6-2-8-9-14-18-14-4 0-8 1-11 4l-3-6c4-3 9-4 14-4z" />
        <circle cx="78" cy="36" r="2.5" fill="#F8FAFC" />
        <path fill="#38BDF8" d="M88 40l6 5-7 2 1-7z" />
      </IconFrame>
    ),
  },
  {
    id: 9,
    name: "Wingspan",
    vibe: "Spread wings from above — energetic, distinctive at a glance.",
    file: "design/icon-options/09-wingspan.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#0891B2" />
        <path fill="#0F172A" d="M64 72c-20 0-36-12-44-28 8 6 18 10 28 10h32c10 0 20-4 28-10-8 16-24 28-44 28z" />
        <path fill="#F8FAFC" d="M64 72c-14 0-26-8-32-20 6 4 14 6 22 6h20c8 0 16-2 22-6-6 12-18 20-32 20z" />
        <ellipse cx="64" cy="58" rx="10" ry="12" fill="#0F172A" />
        <circle cx="68" cy="54" r="2.5" fill="#F8FAFC" />
        <path fill="#67E8F9" d="M64 46c-6-8-2-18 6-22 4-2 8-2 12 0 8 4 12 14 6 22-4-6-10-10-18-10s-14 4-18 10z" />
      </IconFrame>
    ),
  },
  {
    id: 10,
    name: "Chirp",
    vibe: "Friendly mascot head — most playful; great if AI faces are in the roadmap.",
    file: "design/icon-options/10-chirp.svg",
    svg: (
      <IconFrame>
        <rect width="128" height="128" rx="28" fill="#7C3AED" />
        <circle cx="64" cy="66" r="34" fill="#F8FAFC" />
        <circle cx="64" cy="66" r="28" fill="#0F172A" />
        <circle cx="72" cy="58" r="8" fill="#F8FAFC" />
        <circle cx="74" cy="56" r="4" fill="#0F172A" />
        <path fill="#F8FAFC" d="M48 78c4 8 12 14 22 14h-4c-8 0-14-4-18-10v-4z" />
        <path fill="#A78BFA" d="M38 52c-6 4-10 10-10 16 0 2 0 4 1 6l8-4c0-3 2-6 5-8l-4-10z" />
        <path fill="#A78BFA" d="M90 52c6 4 10 10 10 16 0 2 0 4-1 6l-8-4c0-3-2-6-5-8l4-10z" />
        <path fill="#FDE047" d="M86 44l5 4-6 2 1-6z" />
      </IconFrame>
    ),
  },
];

function IconCard({
  option,
  selected,
  onSelect,
}: {
  option: IconOption;
  selected: boolean;
  onSelect: (id: number) => void;
}) {
  const theme = useHostTheme();
  return (
    <button
      type="button"
      onClick={() => onSelect(option.id)}
      style={{
        all: "unset",
        cursor: "pointer",
        display: "block",
        width: "100%",
      }}
    >
      <Card
        variant="default"
        style={{
          border: selected
            ? `2px solid ${theme.accent.primary}`
            : `1px solid ${theme.stroke.secondary}`,
          background: selected ? theme.fill.tertiary : theme.bg.elevated,
        }}
      >
        <CardHeader
          trailing={
            selected ? (
              <Pill size="sm" tone="success">
                Selected
              </Pill>
            ) : (
              <Pill size="sm">#{option.id}</Pill>
            )
          }
        >
          <Text weight="semibold">
            {option.id}. {option.name}
          </Text>
        </CardHeader>
        <CardBody>
          <Stack gap={12} align="center">
            {option.svg}
            <Text size="small" style={{ color: theme.text.secondary, textAlign: "center" }}>
              {option.vibe}
            </Text>
          </Stack>
        </CardBody>
      </Card>
    </button>
  );
}

export default function MagpieIconPicker() {
  const theme = useHostTheme();
  const dispatch = useCanvasAction();
  const [selected, setSelected] = useCanvasState<number | null>("selectedIcon", null);
  const picked = icons.find((i) => i.id === selected);

  return (
    <Stack gap={20}>
      <Stack gap={8}>
        <Row gap={10} align="center" wrap>
          <H1>Magpie app icon — pick your favorite</H1>
          <Pill size="sm">10 concepts</Pill>
        </Row>
        <Text style={{ color: theme.text.secondary, maxWidth: 720 }}>
          Click a card to select it, then reply in chat with the number (e.g.{" "}
          <Text weight="semibold" as="span">"3"</Text> or{" "}
          <Text weight="semibold" as="span">"Gem"</Text>). I'll wire the winner into{" "}
          <Code>src-tauri/icons/</Code>, the window chrome, and the docs.
        </Text>
      </Stack>

      {picked && (
        <Card variant="default">
          <CardBody>
            <Row gap={16} align="center" wrap>
              {picked.svg}
              <Stack gap={4}>
                <Text weight="semibold">
                  Current pick: #{picked.id} {picked.name}
                </Text>
                <Text size="small" style={{ color: theme.text.secondary }}>
                  {picked.file}
                </Text>
                <Button
                  variant="primary"
                  onClick={() =>
                    dispatch({
                      type: "newComposerChat",
                      userPrompt: `Apply icon option ${picked.id} (${picked.name}) from design/icon-options/${String(picked.id).padStart(2, "0")}-${picked.name.toLowerCase()}.svg to the Magpie app (Tauri icons, favicon, TopBar badge).`,
                    })
                  }
                >
                  Ask agent to apply this icon
                </Button>
              </Stack>
            </Row>
          </CardBody>
        </Card>
      )}

      <H2>All options</H2>
      <Grid columns={2} gap={14}>
        {icons.map((option) => (
          <IconCard
            key={option.id}
            option={option}
            selected={selected === option.id}
            onSelect={setSelected}
          />
        ))}
      </Grid>
    </Stack>
  );
}

function Code({ children }: { children: string }) {
  const theme = useHostTheme();
  return (
    <code
      style={{
        fontFamily: "monospace",
        fontSize: "12px",
        padding: "1px 6px",
        borderRadius: 4,
        background: theme.fill.tertiary,
        color: theme.text.primary,
      }}
    >
      {children}
    </code>
  );
}
