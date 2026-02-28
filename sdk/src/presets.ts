export const Presets = {
  SSS_1: 0,
  SSS_2: 1,
} as const;

export const PresetNames = {
  0: 'SSS_1',
  1: 'SSS_2',
} as const;

export const PresetDescriptions: Record<number, string> = {
  0: 'Non-compliant mode - basic stablecoin operations',
  1: 'Compliant mode - blacklist, freeze, seize, transfer hook',
};
