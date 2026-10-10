# Extraction UI review

The Impeccable finishing reviewer inspected the complete 1280×720 empty desktop workspace. It confirmed the central viewer hierarchy, navigation/inspector, Inter typography, pinned dark tokens, lime primary actions, and truthful empty states. Its initial capture rejection found a narrower screenshot with clipping; that capture was replaced at the supported desktop viewport.

One material finding was corrected: the global Space shortcut intercepted focused buttons and enabled playback without a frame. The handler now preserves button/link activation and requires an available frame. A regression test verifies Choose a video, Settings navigation, and disabled empty playback. The reviewer scored the fix resolved, with disposition **ship** for this scoped empty-workspace review.

This is not a full Figma pixel-fidelity or Windows review. Native smoke testing separately confirmed loaded-video preview, thumbnails, frame stepping, selection, and graphical PNG export. Captures are workspace.png and native-workspace.png. The visible video is synthetic FFmpeg testsrc2 media, not user photography or model output.
