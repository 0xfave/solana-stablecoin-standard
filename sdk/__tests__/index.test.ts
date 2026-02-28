import { SolanaStablecoin, PRESET, Presets } from './src/index';

describe('SolanaStablecoin', () => {
  describe('Presets', () => {
    it('should have SSS_1 preset', () => {
      expect(Presets.SSS_1).toBe(0);
    });

    it('should have SSS_2 preset', () => {
      expect(Presets.SSS_2).toBe(1);
    });
  });

  describe('SolanaStablecoin class', () => {
    it('should export SolanaStablecoin class', () => {
      expect(SolanaStablecoin).toBeDefined();
    });

    it('should have create static method', () => {
      expect(typeof SolanaStablecoin.create).toBe('function');
    });

    it('should have fetch static method', () => {
      expect(typeof SolanaStablecoin.fetch).toBe('function');
    });
  });
});
