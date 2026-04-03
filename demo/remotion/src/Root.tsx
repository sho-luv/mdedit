import "./index.css";
import { Composition } from "remotion";
import { MyComposition } from "./Composition";

// Total: 90+150+180+150+120+90 = 780 frames
// Minus 5 transitions * 15 frames = 75 frames overlap
// Net: 705 frames = 23.5 seconds at 30fps
export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="MdeditDemo"
        component={MyComposition}
        durationInFrames={705}
        fps={30}
        width={1280}
        height={720}
      />
    </>
  );
};
