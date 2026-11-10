// Web MIDI access: port lists, one active input/output pair, SysEx enabled.
export async function openMidi(onMessage) {
  if (!navigator.requestMIDIAccess) throw new Error('Web MIDI is not available in this browser (use Chrome or Edge).');
  let access;
  try { access = await navigator.requestMIDIAccess({ sysex: true }); }
  catch (e) { throw new Error(`MIDI access denied: ${e?.message || e?.name || e}`); }

  const api = { inputs: [], outputs: [], onChange: null, use, send };
  let input = null, output = null;
  const list = (m) => [...m.values()].filter((p) => p.state === 'connected').map((p) => ({ id: p.id, name: p.name, state: p.state }));
  const refresh = () => { api.inputs = list(access.inputs); api.outputs = list(access.outputs); };
  refresh();
  access.onstatechange = () => { refresh(); api.onChange?.(); };

  function use(inputId, outputId) {
    if (input) input.onmidimessage = null; // only one listener at a time
    input = access.inputs.get(inputId) ?? null;
    output = access.outputs.get(outputId) ?? null;
    if (input) input.onmidimessage = (e) => onMessage([...e.data]);
  }
  function send(bytes) { if (!output) return false; output.send(bytes); return true; }
  return api;
}
