import { describe, it, expect } from 'vitest';
import { stripTriggers } from './triggerStrip';

describe('stripTriggers', () => {
  it('removes a single completed trigger', () => {
    expect(stripTriggers('>>>[CREATE "a.ts"]<<<')).toBe('');
  });

  it('leaves surrounding plain text intact', () => {
    expect(stripTriggers('Here is your file:\n>>>[CREATE "test.ts"]<<<\nDone!'))
      .toBe('Here is your file:\n\nDone!');
  });

  it('hides an unterminated trailing trigger while streaming', () => {
    const partial = 'Sure, creating it now.\n>>>[WRITE "file.ts" "const x = 1';
    expect(stripTriggers(partial)).toBe('Sure, creating it now.');
  });

  it('does not end the trigger early on a bracket inside quoted content', () => {
    const input = '>>>[WRITE "file.ts" "data]<<<more"]<<<after';
    expect(stripTriggers(input)).toBe('after');
  });

  it('removes multiple triggers across a response', () => {
    const input = '>>>[CREATE "a.ts"]<<<\n>>>[WRITE "a.ts" "content"]<<<\n>>>[MESSAGE "Done"]<<<';
    expect(stripTriggers(input)).toBe('');
  });

  it('is a no-op for text with no triggers', () => {
    expect(stripTriggers('Just a normal message.')).toBe('Just a normal message.');
  });

  // Regression coverage for weaker models (e.g. Mistral Small) that
  // sometimes drop the ">>>" prefix and emit a bare "[ACTION ...]<<<" —
  // without this, the near-miss syntax leaked into the chat as raw text.
  it('hides a bare trigger missing the ">>>" prefix', () => {
    expect(stripTriggers('[CREATE "coucou.md"]<<<')).toBe('');
  });

  it('hides a bare trigger mixed with surrounding text', () => {
    const input = 'Bien sur !\n[WRITE "coucou.md" "contenu"]<<<\nVoila.';
    expect(stripTriggers(input)).toBe('Bien sur !\n\nVoila.');
  });

  it('does not treat ordinary markdown brackets as triggers', () => {
    expect(stripTriggers('See [here](https://example.com) for details'))
      .toBe('See [here](https://example.com) for details');
    expect(stripTriggers('- [ ] todo item')).toBe('- [ ] todo item');
    expect(stripTriggers('As shown [1] in the appendix')).toBe('As shown [1] in the appendix');
  });
});
