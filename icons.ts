import icon0 from "./assets/icons/bluetooth.svg" with { type: "text" };
import icon1 from "./assets/icons/bluetooth-off.svg" with { type: "text" };
import icon2 from "./assets/icons/arrow-left-right.svg" with { type: "text" };
import icon3 from "./assets/icons/clock-3.svg" with { type: "text" };
import icon4 from "./assets/icons/settings-2.svg" with { type: "text" };
import icon5 from "./assets/icons/laptop.svg" with { type: "text" };
import icon6 from "./assets/icons/plus.svg" with { type: "text" };
import icon7 from "./assets/icons/search.svg" with { type: "text" };
import icon8 from "./assets/icons/headphones.svg" with { type: "text" };
import icon9 from "./assets/icons/mouse.svg" with { type: "text" };
import icon10 from "./assets/icons/keyboard.svg" with { type: "text" };
import icon11 from "./assets/icons/smartphone.svg" with { type: "text" };
import icon12 from "./assets/icons/speaker.svg" with { type: "text" };
import icon13 from "./assets/icons/gamepad-2.svg" with { type: "text" };
import icon14 from "./assets/icons/battery-medium.svg" with { type: "text" };
import icon15 from "./assets/icons/ellipsis.svg" with { type: "text" };
import icon16 from "./assets/icons/radar.svg" with { type: "text" };
import icon17 from "./assets/icons/eye-off.svg" with { type: "text" };
import icon18 from "./assets/icons/eye.svg" with { type: "text" };
import icon19 from "./assets/icons/chevron-down.svg" with { type: "text" };
import icon20 from "./assets/icons/volume-2.svg" with { type: "text" };
import icon21 from "./assets/icons/volume-x.svg" with { type: "text" };
import icon22 from "./assets/icons/minus.svg" with { type: "text" };
import icon23 from "./assets/icons/square.svg" with { type: "text" };
import icon24 from "./assets/icons/x.svg" with { type: "text" };
import icon25 from "./assets/icons/link.svg" with { type: "text" };
import icon26 from "./assets/icons/circle-check.svg" with { type: "text" };
import icon27 from "./assets/icons/circle.svg" with { type: "text" };
import icon28 from "./assets/icons/info.svg" with { type: "text" };
import icon29 from "./assets/icons/check.svg" with { type: "text" };
import icon30 from "./assets/icons/folder.svg" with { type: "text" };
import icon31 from "./assets/icons/file.svg" with { type: "text" };
import icon32 from "./assets/icons/arrow-up-right.svg" with { type: "text" };
import icon33 from "./assets/icons/shield-check.svg" with { type: "text" };
export const icons = {
  bluetooth: icon0,
  "bluetooth-off": icon1,
  "arrow-left-right": icon2,
  "clock-3": icon3,
  "settings-2": icon4,
  laptop: icon5,
  plus: icon6,
  search: icon7,
  headphones: icon8,
  mouse: icon9,
  keyboard: icon10,
  smartphone: icon11,
  speaker: icon12,
  "gamepad-2": icon13,
  "battery-medium": icon14,
  ellipsis: icon15,
  radar: icon16,
  "eye-off": icon17,
  eye: icon18,
  "chevron-down": icon19,
  "volume-2": icon20,
  "volume-x": icon21,
  minus: icon22,
  square: icon23,
  x: icon24,
  link: icon25,
  "circle-check": icon26,
  circle: icon27,
  info: icon28,
  check: icon29,
  folder: icon30,
  file: icon31,
  "arrow-up-right": icon32,
  "shield-check": icon33,
} as const;
export type IconName = keyof typeof icons;
