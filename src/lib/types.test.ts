import { describe, it, expect } from 'vitest';
import { MODEL_OPTIONS, PROVIDER_BADGE } from './types';

describe('MODEL_OPTIONS', () => {
  it('contains all required providers', () => {
    const providers = new Set(MODEL_OPTIONS.map(m => m.provider));
    expect(providers.has('openai')).toBe(true);
    expect(providers.has('anthropic')).toBe(true);
    expect(providers.has('google')).toBe(true);
    expect(providers.has('ollama')).toBe(true);
  });

  it('has unique provider+id combinations', () => {
    // Note: the same model `id` can legitimately appear under two different
    // `provider`s (e.g. a model under both 'anthropic' (API key auth) and
    // 'anthropic-oauth' (reused CLI session auth) — same underlying model,
    // different auth path) so uniqueness is scoped to the provider+id pair,
    // which is what's actually used to dispatch a request.
    const keys = MODEL_OPTIONS.map(m => `${m.provider}:${m.id}`);
    const unique = new Set(keys);
    expect(unique.size).toBe(keys.length);
  });

  it('every model has a name and provider', () => {
    for (const m of MODEL_OPTIONS) {
      expect(m.name).toBeTruthy();
      expect(m.provider).toBeTruthy();
      expect(m.id).toBeTruthy();
    }
  });

  it('every provider used by a model has a PROVIDER_BADGE entry', () => {
    const providers = new Set(MODEL_OPTIONS.map(m => m.provider));
    for (const provider of providers) {
      expect(PROVIDER_BADGE[provider], `missing PROVIDER_BADGE for "${provider}"`).toBeTruthy();
    }
  });
});
