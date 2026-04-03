import { TransitionSeries, linearTiming } from "@remotion/transitions";
import { fade } from "@remotion/transitions/fade";
import { IntroScene } from "./scenes/IntroScene";
import { ProblemScene } from "./scenes/ProblemScene";
import { DemoScene } from "./scenes/DemoScene";
import { FeaturesScene } from "./scenes/FeaturesScene";
import { InstallScene } from "./scenes/InstallScene";
import { CTAScene } from "./scenes/CTAScene";

export const MyComposition: React.FC = () => {
  return (
    <TransitionSeries>
      {/* Scene 1: Logo intro (3s) */}
      <TransitionSeries.Sequence durationInFrames={90}>
        <IntroScene />
      </TransitionSeries.Sequence>

      <TransitionSeries.Transition
        presentation={fade()}
        timing={linearTiming({ durationInFrames: 15 })}
      />

      {/* Scene 2: Problem statement (5s) */}
      <TransitionSeries.Sequence durationInFrames={150}>
        <ProblemScene />
      </TransitionSeries.Sequence>

      <TransitionSeries.Transition
        presentation={fade()}
        timing={linearTiming({ durationInFrames: 15 })}
      />

      {/* Scene 3: Terminal demo (6s) */}
      <TransitionSeries.Sequence durationInFrames={180}>
        <DemoScene />
      </TransitionSeries.Sequence>

      <TransitionSeries.Transition
        presentation={fade()}
        timing={linearTiming({ durationInFrames: 15 })}
      />

      {/* Scene 4: Features (5s) */}
      <TransitionSeries.Sequence durationInFrames={150}>
        <FeaturesScene />
      </TransitionSeries.Sequence>

      <TransitionSeries.Transition
        presentation={fade()}
        timing={linearTiming({ durationInFrames: 15 })}
      />

      {/* Scene 5: Install (4s) */}
      <TransitionSeries.Sequence durationInFrames={120}>
        <InstallScene />
      </TransitionSeries.Sequence>

      <TransitionSeries.Transition
        presentation={fade()}
        timing={linearTiming({ durationInFrames: 15 })}
      />

      {/* Scene 6: CTA (3s) */}
      <TransitionSeries.Sequence durationInFrames={90}>
        <CTAScene />
      </TransitionSeries.Sequence>
    </TransitionSeries>
  );
};
