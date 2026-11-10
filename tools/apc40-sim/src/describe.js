// Human-readable decoding of a raw MIDI message (channels shown 1-based).
const hex = (b) => b.map((x) => x.toString(16).toUpperCase().padStart(2, '0')).join(' ');
const SYS = { 0xf8: 'Clock', 0xfa: 'Start', 0xfb: 'Continue', 0xfc: 'Stop', 0xfe: 'Active Sensing' };

export function describe(bytes) {
  try {
    if (!bytes?.length) return 'Other';
    const [s, a, b] = bytes, ch = (s & 0x0f) + 1;
    if (SYS[s]) return SYS[s];
    if (s === 0xf0) return `SysEx ${hex(bytes)} (${bytes.length} bytes)`;
    if (s >= 0x80 && s < 0xa0 && bytes.length === 3)
      return `Note ${s >= 0x90 ? 'On' : 'Off'} ch${ch} #${a} vel${b}`;
    if (s >= 0xb0 && s < 0xc0 && bytes.length === 3) return `CC ch${ch} #${a} = ${b}`;
    return `Other ${hex(bytes)}`;
  } catch { return 'Other'; }
}
