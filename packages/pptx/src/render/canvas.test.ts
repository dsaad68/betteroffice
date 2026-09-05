import { describe, expect, test } from 'bun:test';
import type { GeometryPathCommand, SlideDisplayList } from '../types';
import { paintSlide } from './canvas';

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

describe('PPTX picture cropping', () => {
  function harness() {
    const calls: string[] = [];
    const ctx = new Proxy(
      {
        drawImage: (...args: unknown[]) => calls.push(`draw:${args.slice(1).join(',')}`),
        clip: () => calls.push('clip'),
        save: () => calls.push('save'),
        restore: () => calls.push('restore'),
        beginPath: () => calls.push('beginPath'),
        rect: (...args: unknown[]) => calls.push(`rect:${args.join(',')}`),
        moveTo: (...args: unknown[]) => calls.push(`move:${args.join(',')}`),
        lineTo: (...args: unknown[]) => calls.push(`line:${args.join(',')}`),
        bezierCurveTo: (...args: unknown[]) => calls.push(`cubic:${args.join(',')}`),
        closePath: () => calls.push('close'),
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
    return { calls, ctx };
  }

  function list(image: Record<string, unknown>): SlideDisplayList {
    return {
      contractVersion: 1,
      width: 320,
      height: 180,
      primitives: [
        {
          kind: 'image',
          objectId: 1,
          name: 'Screenshot',
          x: 10,
          y: 20,
          w: 200,
          h: 100,
          assetId: 'ppt/media/image1.png',
          ...image,
        },
      ],
    } as SlideDisplayList;
  }

  const source = { width: 400, height: 300 } as unknown as CanvasImageSource;

  const ellipse: GeometryPathCommand[] = [
    { type: 'move', x: 1, y: 0.5 },
    { type: 'cubic', cp1x: 1, cp1y: 0.75, cp2x: 0.75, cp2y: 1, x: 0.5, y: 1 },
    { type: 'cubic', cp1x: 0.25, cp1y: 1, cp2x: 0, cp2y: 0.75, x: 0, y: 0.5 },
    { type: 'cubic', cp1x: 0, cp1y: 0.25, cp2x: 0.25, cp2y: 0, x: 0.5, y: 0 },
    { type: 'cubic', cp1x: 0.75, cp1y: 0, cp2x: 1, cp2y: 0.25, x: 1, y: 0.5 },
    { type: 'close' },
  ];

  const ellipseOutline = [
    'beginPath',
    'move:210,70',
    'cubic:210,95,160,120,110,120',
    'cubic:60,120,10,95,10,70',
    'cubic:10,45,60,20,110,20',
    'cubic:160,20,210,45,210,70',
    'close',
  ];

  test('a cropped picture with its own outline is clipped and stroked along that outline', async () => {
    const { calls, ctx } = harness();
    await paintSlide(
      ctx,
      list({
        crop: { left: 0.25, top: 0.5, right: 0.25, bottom: 0.25 },
        path: ellipse,
        stroke: { color: '#ff00ff', width: 2 },
      }),
      1,
      1,
      { resolveImage: async () => source }
    );
    expect(calls).toEqual([
      'save',
      'save',
      'save',
      ...ellipseOutline,
      'clip',
      'draw:100,150,200,75,10,20,200,100',
      'restore',
      ...ellipseOutline,
      'stroke',
      'restore',
      'restore',
    ]);
  });

  test('an uncropped picture with its own outline still draws the whole source through it', async () => {
    const { calls, ctx } = harness();
    await paintSlide(ctx, list({ path: ellipse }), 1, 1, { resolveImage: async () => source });
    expect(calls).toEqual([
      'save',
      'save',
      'save',
      ...ellipseOutline,
      'clip',
      'draw:0,0,400,300,10,20,200,100',
      'restore',
      'restore',
      'restore',
    ]);
  });

  test('a picture without its own outline is stroked along its frame', async () => {
    const { calls, ctx } = harness();
    await paintSlide(ctx, list({ stroke: { color: '#10235b', width: 1 } }), 1, 1, {
      resolveImage: async () => source,
    });
    expect(calls).toEqual([
      'save',
      'save',
      'draw:0,0,400,300,10,20,200,100',
      'beginPath',
      'rect:10,20,200,100',
      'stroke',
      'restore',
      'restore',
    ]);
  });

  test('draws only the kept sub-rectangle, masked to the frame', async () => {
    const { calls, ctx } = harness();
    await paintSlide(ctx, list({ crop: { top: 0.1, bottom: 0.2 } }), 1, 1, {
      resolveImage: async () => source,
    });
    expect(calls).toContain('save');
    expect(calls).toContain('clip');
    expect(calls).toContain('restore');
    expect(calls).toContain('draw:0,30,400,210,10,20,200,100');
  });

  test('an uncropped picture draws the whole source and needs no mask', async () => {
    const { calls, ctx } = harness();
    await paintSlide(ctx, list({}), 1, 1, { resolveImage: async () => source });
    expect(calls).toContain('draw:0,0,400,300,10,20,200,100');
    expect(calls).not.toContain('clip');
  });

  test('a crop that keeps nothing draws nothing', async () => {
    const { calls, ctx } = harness();
    await paintSlide(ctx, list({ crop: { left: 0.6, right: 0.6 } }), 1, 1, {
      resolveImage: async () => source,
    });
    expect(calls.some((call) => call.startsWith('draw:'))).toBe(false);
  });
});
