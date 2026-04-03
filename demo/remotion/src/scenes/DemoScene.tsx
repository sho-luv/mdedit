import { useCurrentFrame, useVideoConfig, interpolate, spring } from "remotion";

// Simulated terminal content - left pane (raw markdown) and right pane (rendered)
const editorLines = [
  { text: "# Getting Started", color: "#7ee787" },
  { text: "", color: "#e6edf3" },
  { text: "Welcome to **mdedit**, a terminal", color: "#e6edf3" },
  { text: "markdown editor with live preview.", color: "#e6edf3" },
  { text: "", color: "#e6edf3" },
  { text: "## Features", color: "#7ee787" },
  { text: "", color: "#e6edf3" },
  { text: "- Side-by-side editing", color: "#e6edf3" },
  { text: "- Vim keybindings", color: "#e6edf3" },
  { text: "- Syntax highlighting", color: "#e6edf3" },
  { text: "- Works over SSH", color: "#e6edf3" },
  { text: "", color: "#e6edf3" },
  { text: "```rust", color: "#8b949e" },
  { text: "fn main() {", color: "#ff7b72" },
  { text: '    println!("Hello!");', color: "#a5d6ff" },
  { text: "}", color: "#ff7b72" },
  { text: "```", color: "#8b949e" },
];

const previewLines = [
  { text: "Getting Started", size: 32, color: "#58a6ff", weight: 700, marginBottom: 12 },
  { text: "Welcome to mdedit, a terminal markdown", size: 18, color: "#e6edf3", weight: 400, marginBottom: 2 },
  { text: "editor with live preview.", size: 18, color: "#e6edf3", weight: 400, marginBottom: 20 },
  { text: "Features", size: 26, color: "#58a6ff", weight: 700, marginBottom: 10 },
  { text: "  \u2022 Side-by-side editing", size: 18, color: "#e6edf3", weight: 400, marginBottom: 4 },
  { text: "  \u2022 Vim keybindings", size: 18, color: "#e6edf3", weight: 400, marginBottom: 4 },
  { text: "  \u2022 Syntax highlighting", size: 18, color: "#e6edf3", weight: 400, marginBottom: 4 },
  { text: "  \u2022 Works over SSH", size: 18, color: "#e6edf3", weight: 400, marginBottom: 16 },
];

export const DemoScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const terminalScale = spring({
    frame,
    fps,
    config: { damping: 200 },
  });

  const terminalOpacity = interpolate(frame, [0, 0.5 * fps], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  // Typing effect - reveal lines over time
  const typingStart = 0.8 * fps;
  const linesPerSecond = 4;
  const visibleLines = Math.min(
    Math.floor(
      interpolate(frame, [typingStart, typingStart + (editorLines.length / linesPerSecond) * fps], [0, editorLines.length], {
        extrapolateLeft: "clamp",
        extrapolateRight: "clamp",
      }),
    ),
    editorLines.length,
  );

  // Cursor blink
  const cursorOpacity = Math.round((frame * 2) / fps) % 2 === 0 ? 1 : 0;

  // Status bar label
  const labelOpacity = interpolate(
    frame,
    [0.3 * fps, 0.6 * fps],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
  );

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full"
      style={{ backgroundColor: "#0d1117" }}
    >
      <p
        style={{
          fontSize: 24,
          color: "#7ee787",
          fontFamily: "system-ui, sans-serif",
          fontWeight: 600,
          letterSpacing: 3,
          textTransform: "uppercase",
          marginBottom: 24,
          opacity: labelOpacity,
        }}
      >
        Edit + Preview in one terminal
      </p>

      <div
        style={{
          transform: `scale(${terminalScale})`,
          opacity: terminalOpacity,
          borderRadius: 12,
          overflow: "hidden",
          boxShadow: "0 20px 60px rgba(0,0,0,0.6)",
          width: 1000,
          backgroundColor: "#161b22",
          border: "1px solid #30363d",
        }}
      >
        {/* Title bar */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            padding: "10px 16px",
            backgroundColor: "#0d1117",
            borderBottom: "1px solid #30363d",
            gap: 8,
          }}
        >
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#f85149" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#e3b341" }} />
          <div style={{ width: 12, height: 12, borderRadius: 6, backgroundColor: "#7ee787" }} />
          <span style={{ marginLeft: 12, fontSize: 13, color: "#8b949e", fontFamily: "monospace" }}>
            mdedit README.md
          </span>
        </div>

        {/* Split pane content */}
        <div style={{ display: "flex", minHeight: 380 }}>
          {/* Editor pane */}
          <div style={{ flex: 1, padding: "12px 16px", borderRight: "1px solid #30363d" }}>
            {editorLines.slice(0, visibleLines).map((line, i) => (
              <div key={i} style={{ display: "flex", alignItems: "center", height: 22 }}>
                <span style={{ width: 30, fontSize: 13, color: "#484f58", fontFamily: "monospace", textAlign: "right", marginRight: 12 }}>
                  {i + 1}
                </span>
                <span style={{ fontSize: 14, color: line.color, fontFamily: "monospace" }}>
                  {line.text}
                </span>
                {i === visibleLines - 1 && (
                  <span style={{ fontSize: 14, color: "#7ee787", fontFamily: "monospace", opacity: cursorOpacity }}>
                    |
                  </span>
                )}
              </div>
            ))}
          </div>

          {/* Preview pane */}
          <div style={{ flex: 1, padding: "12px 20px" }}>
            {previewLines.map((line, i) => {
              const lineDelay = typingStart + (i * 2 / linesPerSecond) * fps;
              const lineOpacity = interpolate(frame, [lineDelay, lineDelay + 8], [0, 1], {
                extrapolateLeft: "clamp",
                extrapolateRight: "clamp",
              });
              return (
                <p
                  key={i}
                  style={{
                    fontSize: line.size,
                    color: line.color,
                    fontWeight: line.weight,
                    fontFamily: line.size > 20 ? "system-ui, sans-serif" : "system-ui, sans-serif",
                    marginBottom: line.marginBottom,
                    opacity: lineOpacity,
                    lineHeight: 1.4,
                  }}
                >
                  {line.text}
                </p>
              );
            })}

            {/* Code block in preview */}
            {visibleLines >= 14 && (
              <div
                style={{
                  backgroundColor: "#0d1117",
                  borderRadius: 8,
                  padding: "10px 14px",
                  marginTop: 8,
                  opacity: interpolate(frame, [typingStart + (13 / linesPerSecond) * fps, typingStart + (15 / linesPerSecond) * fps], [0, 1], {
                    extrapolateLeft: "clamp",
                    extrapolateRight: "clamp",
                  }),
                }}
              >
                <p style={{ fontSize: 14, color: "#ff7b72", fontFamily: "monospace", margin: 0, lineHeight: 1.6 }}>
                  fn <span style={{ color: "#d2a8ff" }}>main</span>() {"{"}
                </p>
                <p style={{ fontSize: 14, color: "#a5d6ff", fontFamily: "monospace", margin: 0, paddingLeft: 20, lineHeight: 1.6 }}>
                  println!(<span style={{ color: "#a5d6ff" }}>"Hello!"</span>);
                </p>
                <p style={{ fontSize: 14, color: "#ff7b72", fontFamily: "monospace", margin: 0, lineHeight: 1.6 }}>
                  {"}"}
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Status bar */}
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            padding: "6px 16px",
            backgroundColor: "#0d1117",
            borderTop: "1px solid #30363d",
          }}
        >
          <span style={{ fontSize: 12, color: "#7ee787", fontFamily: "monospace" }}>NORMAL</span>
          <span style={{ fontSize: 12, color: "#8b949e", fontFamily: "monospace" }}>README.md</span>
          <span style={{ fontSize: 12, color: "#8b949e", fontFamily: "monospace" }}>Ln {visibleLines}, Col 1</span>
        </div>
      </div>
    </div>
  );
};
