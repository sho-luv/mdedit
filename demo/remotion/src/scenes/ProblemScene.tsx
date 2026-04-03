import { useCurrentFrame, interpolate } from "remotion";

const statements = [
  { text: "Markdown editors that need a browser", color: "#e6edf3" },
  { text: "VS Code just to edit a README", color: "#f85149" },
  { text: "Obsidian locks your notes in a vault", color: "#7ee787" },
  { text: "No live preview over SSH", color: "#e6edf3" },
  { text: "You live in the terminal. Your editor should too.", color: "#58a6ff" },
];

export const ProblemScene: React.FC = () => {
  const frame = useCurrentFrame();

  const framePer = 30;
  const fadeIn = 8;
  const fadeOut = 8;

  return (
    <div
      className="flex flex-col items-center justify-center w-full h-full px-20"
      style={{ backgroundColor: "#0d1117" }}
    >
      {statements.map((s, i) => {
        const start = i * framePer;
        const end = start + framePer;

        const opacity = interpolate(
          frame,
          [start, start + fadeIn, end - fadeOut, end],
          [0, 1, 1, 0],
          { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
        );

        const y = interpolate(
          frame,
          [start, start + fadeIn],
          [20, 0],
          { extrapolateLeft: "clamp", extrapolateRight: "clamp" },
        );

        return (
          <p
            key={i}
            style={{
              position: "absolute",
              fontSize: 48,
              fontWeight: 700,
              color: s.color,
              fontFamily: "system-ui, sans-serif",
              textAlign: "center",
              maxWidth: 1000,
              opacity,
              transform: `translateY(${y}px)`,
            }}
          >
            {s.text}
          </p>
        );
      })}
    </div>
  );
};
