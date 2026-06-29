// Shared UI chrome icons (Phosphor via react-icons/pi). Keeps the app's
// non-node iconography on one consistent line set instead of emoji.
export {
  PiSun as IconSun,
  PiMoonStars as IconMoon,
  PiBell as IconBell,
  PiKey as IconKey,
  PiLock as IconLock,
  PiGlobe as IconGlobe,
  PiFolder as IconFolder,
  PiTrash as IconTrash,
  PiBuildings as IconBuildings,
  PiFloppyDisk as IconSave,
  PiMagnifyingGlass as IconSearch,
  PiTestTube as IconTestTube,
  // Toolbar / nav / status chrome — replaces raw glyphs (☰ ⚙ ⚡ ✋ ⚠ ℹ ✓ ✗ ▶).
  PiList as IconMenu,
  PiListDashes as IconViewList,
  PiGridFour as IconViewGrid,
  PiColumns as IconColumns,
  PiLightning as IconActivity,
  PiInfo as IconInfo,
  PiWarning as IconWarning,
  PiHandPalm as IconApproval,
  PiCheck as IconCheck,
  PiX as IconX,
  PiCards as IconCards,
  PiPlay as IconPlay,
  PiTable as IconTable,
  PiKanban as IconKanban,
  PiTimer as IconTimer,
  PiGraph as IconGraph,
} from 'react-icons/pi'

import { PiSun, PiMoonStars } from 'react-icons/pi'

/** Theme toggle glyph: sun in dark mode (click → light), moon in light mode. */
export function ThemeToggleIcon({ dark, size = 15 }: { dark: boolean; size?: number }) {
  return dark ? <PiSun size={size} aria-hidden /> : <PiMoonStars size={size} aria-hidden />
}
