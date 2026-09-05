import { describe, expect, test } from 'bun:test';
import type { ImageEffect, SlideDisplayList } from '../types';
import { applyImageEffects, paintSlide } from './canvas';

describe('PPTX canvas replay', () => {
  test('paints shape geometry and positioned text in display-list order', async () => {
    const calls: string[] = [];
    const ctx = new Proxy(
      {
        createLinearGradient: () => ({ addColorStop: () => undefined }),
        createRadialGradient: () => ({ addColorStop: () => undefined }),
        fillText: (text: string) => calls.push(`text:${text}`),
        moveTo: () => calls.push('move'),
        lineTo: () => calls.push('line'),
        fill: () => calls.push('fill'),
        stroke: () => calls.push('stroke'),
      } as Record<string, unknown>,
      {
        get(target, property) {
          if (property in target) return target[property as string];
          return () => undefined;
        },
        set(target, property, value) {
          target[property as string] = value;
          return true;
        },
      }
    ) as unknown as CanvasRenderingContext2D;
    const list: SlideDisplayList = {
      contractVersion: 1,
      width: 320,
      height: 180,
      background: { kind: 'solid', color: '#ffffff' },
      primitives: [
        {
          kind: 'shape',
          objectId: 1,
          shapeId: 'shape:1',
          name: 'Card',
          x: 20,
          y: 20,
          w: 280,
          h: 140,
          geometry: 'rect',
          path: [
            { type: 'move', x: 0, y: 0 },
            { type: 'line', x: 1, y: 0 },
            { type: 'line', x: 1, y: 1 },
            { type: 'close' },
          ],
          fill: { kind: 'solid', color: '#325ee6' },
          stroke: { color: '#10235b', width: 2 },
        },
        {
          kind: 'textBox',
          objectId: 1,
          shapeId: 'shape:1',
          storyId: 'story:1',
          x: 40,
          y: 50,
          w: 240,
          h: 80,
          anchor: 'top',
          paragraphs: [],
          lines: [
            {
              x: 40,
              y: 50,
              width: 60,
              height: 24,
              baseline: 68,
              start: 0,
              end: 5,
              caretStops: [
                { position: 0, x: 40 },
                { position: 5, x: 100 },
              ],
              runs: [
                {
                  text: 'Hello',
                  start: 0,
                  end: 5,
                  x: 40,
                  width: 60,
                  fontId: 1,
                  fontFamily: 'Liberation Sans',
                  fontSizePx: 20,
                  bold: false,
                  italic: false,
                  underline: false,
                  color: '#ffffff',
                  glyphs: [],
                },
              ],
            },
          ],
        },
      ],
    };

    await paintSlide(ctx, list, 2);
    expect(calls).toContain('move');
    expect(calls).toContain('line');
    expect(calls).toContain('fill');
    expect(calls).toContain('stroke');
    expect(calls).toContain('text:Hello');
  });

  test('paints chart parts clipped to the chart rectangle', async () => {
    const calls: string[] = [];
    const ctx = new Proxy(
      {
        clip: () => calls.push('clip'),
        fillText: (text: string) => calls.push(`text:${text}`),
        fill: () => calls.push('fill'),
      } as Record<string, unknown>,
      {
        get(target, property) {
          if (property in target) return target[property as string];
          return () => undefined;
        },
        set(target, property, value) {
          target[property as string] = value;
          return true;
        },
      }
    ) as unknown as CanvasRenderingContext2D;
    const list: SlideDisplayList = {
      contractVersion: 1,
      width: 320,
      height: 180,
      primitives: [
        {
          kind: 'chart',
          objectId: 4,
          shapeId: 'slide:0:1',
          name: 'Revenue chart',
          label: 'Revenue, column chart, 2 series, 3 categories',
          x: 10,
          y: 10,
          w: 300,
          h: 160,
          primitives: [
            {
              kind: 'shape',
              objectId: 4,
              name: '',
              x: 20,
              y: 20,
              w: 40,
              h: 100,
              geometry: 'rect',
              path: [
                { type: 'move', x: 0, y: 0 },
                { type: 'line', x: 1, y: 0 },
                { type: 'close' },
              ],
              fill: { kind: 'solid', color: '#6254e7' },
            },
            {
              kind: 'textBox',
              objectId: 4,
              x: 20,
              y: 130,
              w: 60,
              h: 14,
              anchor: 'top',
              paragraphs: [],
              lines: [
                {
                  x: 20,
                  y: 130,
                  width: 20,
                  height: 14,
                  baseline: 141,
                  start: 0,
                  end: 2,
                  caretStops: [],
                  runs: [
                    {
                      text: 'Q1',
                      start: 0,
                      end: 2,
                      x: 20,
                      width: 20,
                      fontId: 1,
                      fontFamily: 'Liberation Sans',
                      fontSizePx: 10,
                      bold: false,
                      italic: false,
                      underline: false,
                      color: '#222222',
                      glyphs: [],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    };

    await paintSlide(ctx, list, 1);
    expect(calls).toContain('clip');
    expect(calls).toContain('fill');
    expect(calls).toContain('text:Q1');
  });

  test('paints justified word starts at the engine caret positions', async () => {
    const calls: Array<{ text: string; x: number }> = [];
    const ctx = new Proxy(
      {
        fillText: (text: string, x: number) => calls.push({ text, x }),
      } as Record<string, unknown>,
      {
        get(target, property) {
          if (property in target) return target[property as string];
          return () => undefined;
        },
        set(target, property, value) {
          target[property as string] = value;
          return true;
        },
      }
    ) as unknown as CanvasRenderingContext2D;
    const caretStops = [
      { position: 0, x: 40 },
      { position: 1, x: 50 },
      { position: 2, x: 60 },
      { position: 3, x: 70 },
      { position: 4, x: 100 },
      { position: 5, x: 110 },
      { position: 6, x: 120 },
      { position: 7, x: 130 },
    ];
    const glyphs = [
      { glyphId: 1, cluster: 0, x: 40, advance: 10, xOffset: 0, yOffset: 68 },
      { glyphId: 2, cluster: 1, x: 50, advance: 10, xOffset: 0, yOffset: 68 },
      { glyphId: 3, cluster: 2, x: 60, advance: 10, xOffset: 0, yOffset: 68 },
      { glyphId: 4, cluster: 3, x: 70, advance: 5, xOffset: 0, yOffset: 68 },
      { glyphId: 5, cluster: 4, x: 100, advance: 10, xOffset: 0, yOffset: 68 },
      { glyphId: 6, cluster: 5, x: 110, advance: 10, xOffset: 0, yOffset: 68 },
      { glyphId: 7, cluster: 6, x: 120, advance: 10, xOffset: 0, yOffset: 68 },
    ];
    const list: SlideDisplayList = {
      contractVersion: 1,
      width: 320,
      height: 180,
      primitives: [
        {
          kind: 'textBox',
          objectId: 1,
          shapeId: 'shape:1',
          storyId: 'story:1',
          x: 40,
          y: 50,
          w: 240,
          h: 80,
          anchor: 'top',
          paragraphs: [],
          lines: [
            {
              x: 40,
              y: 50,
              width: 90,
              height: 24,
              baseline: 68,
              start: 0,
              end: 7,
              caretStops,
              runs: [
                {
                  text: 'one two',
                  start: 0,
                  end: 7,
                  x: 40,
                  width: 90,
                  fontId: 1,
                  fontFamily: 'Liberation Sans',
                  fontSizePx: 20,
                  bold: false,
                  italic: false,
                  underline: false,
                  color: '#ffffff',
                  glyphs,
                },
              ],
            },
          ],
        },
      ],
    };

    await paintSlide(ctx, list, 1);
    const painted = calls.find((call) => call.text === 'two');
    const engine = caretStops.find((stop) => stop.position === 4);
    expect(painted?.x).toBe(engine?.x);
    expect(calls).toEqual([
      { text: 'one ', x: 40 },
      { text: 'two', x: 100 },
    ]);
  });
});

describe('blip colour effects', () => {
  // #03A7DF is the elastica mark from the cisco deck: 48.7% under Rec. 601 and
  // 53.4% under Rec. 709, so the two luma formulas disagree at this threshold.
  test('biLevel thresholds on Rec. 601 luma and leaves alpha alone', () => {
    const data = new Uint8ClampedArray([0x03, 0xa7, 0xdf, 0x80]);
    applyImageEffects(data, [{ kind: 'biLevel', threshold: 0.5 }]);
    expect([...data]).toEqual([0, 0, 0, 0x80]);

    const light = new Uint8ClampedArray([0x03, 0xa7, 0xdf, 0xff]);
    applyImageEffects(light, [{ kind: 'biLevel', threshold: 0.25 }]);
    expect([...light]).toEqual([255, 255, 255, 0xff]);
  });

  test('duotone interpolates between the two colours by luma', () => {
    const data = new Uint8ClampedArray([0, 0, 0, 0xff, 255, 255, 255, 0xff]);
    applyImageEffects(data, [{ kind: 'duotone', shadow: '#737373ff', highlight: '#ffffffff' }]);
    expect([...data]).toEqual([0x73, 0x73, 0x73, 0xff, 255, 255, 255, 0xff]);
  });

  // clrFrom="FFFFFF" -> clrTo with alpha="0" has to run before the duotone that
  // follows it, or the whole bitmap is recoloured opaque.
  test('effects apply in list order', () => {
    const ordered: ImageEffect[] = [
      { kind: 'colorChange', from: '#ffffffff', to: '#ffffff00' },
      { kind: 'duotone', shadow: '#000000ff', highlight: '#ff0000ff' },
    ];
    const data = new Uint8ClampedArray([255, 255, 255, 0xff]);
    applyImageEffects(data, ordered);
    expect(data[3]).toBe(0);

    const reversed = new Uint8ClampedArray([255, 255, 255, 0xff]);
    applyImageEffects(reversed, [...ordered].reverse());
    expect(reversed[3]).toBe(0xff);
  });
});
