<div align="center">

# Variable Blur

**iOS-style variable blur for Node.js**

[![NPM Version](https://img.shields.io/npm/v/variable-blur?style=flat-square)](https://www.npmjs.com/package/variable-blur)

<img src="docs/example-blurred.png" width="500">

</div>

## Installation

```bash
npm install variable-blur
```

Prebuilt binaries for macOS / Linux / Windows (x64 & arm64).

## Quick Start

```js
import { readFile, writeFile } from 'node:fs/promises'
import { variableBlur } from 'variable-blur'

const input = await readFile('photo.jpg')

const output = variableBlur({
  buffer: input,
  options: {
    x: 1,
    y: 0,
    maxSigma: 32,
    quality: 0.5,
  },
})

await writeFile('photo-blurred.jpg', output)
```

`variableBlur` accepts encoded image bytes as input and returns encoded image bytes as output.

## Using With Sharp

```js
import sharp from 'sharp'
import { variableBlurRaw } from 'variable-blur'

const pipeline = sharp('photo.jpg').resize(1400).ensureAlpha()
const { data, info } = await pipeline.raw().toBuffer({ resolveWithObject: true })

const blurred = variableBlurRaw({
  data,
  width: info.width,
  height: info.height,
  channels: info.channels,
  options: {
    x: 1,
    y: 0,
    maxSigma: 32,
    quality: 0.5,
  },
})

const output = await sharp(blurred, {
  raw: {
    width: info.width,
    height: info.height,
    channels: info.channels,
  },
})
  .jpeg()
  .toBuffer()
```

## Debug UI

```bash
cargo run -p variable_blur_debug_ui -r
```

Built-in [egui](https://github.com/emilk/egui) tool for real-time parameter tuning.

## API

### `variableBlur(input): Buffer`

| Parameter       | Type     | Description                           |
| :-------------- | :------- | :------------------------------------ |
| `input.buffer`  | `Buffer` | Encoded image (PNG, JPEG, WebP, etc.) |
| `input.options` | `object` | Required options object; see below    |

### `variableBlurRaw(input): Buffer`

Best for `sharp.raw().toBuffer({ resolveWithObject: true })` output.

| Parameter        | Type                  | Description                                             |
| :--------------- | :-------------------- | :------------------------------------------------------ |
| `input.data`     | `Buffer`              | Interleaved raw pixel bytes                             |
| `input.width`    | `number`              | Image width in pixels                                   |
| `input.height`   | `number`              | Image height in pixels                                  |
| `input.channels` | `3 \| 4`              | Raw channel count; current support is `RGB` or `RGBA`   |
| `input.options`  | `VariableBlurOptions` | Required options object; same shape as `variableBlur()` |

### Options

| Field          | Type     | Optional | Default        | Description                                                                                   |
| :------------- | :------- | :------: | :------------- | :-------------------------------------------------------------------------------------------- |
| `x`            | `number` |    no    | -              | Finite X component of the blur direction vector                                               |
| `y`            | `number` |    no    | -              | Finite Y component of the blur direction vector                                               |
| `start`        | `number` |   yes    | auto           | Finite projection coordinate where blur begins                                                |
| `end`          | `number` |   yes    | auto           | Finite projection coordinate where blur reaches max                                           |
| `quality`      | `number` |   yes    | `0.5`          | Quality factor in `[0, 1]`; higher values use more sigma anchors and shallower pyramid levels |
| `maxSigma`     | `number` |    no    | -              | Maximum Gaussian sigma; controls the blur strength cap                                        |
| `curve`        | `string` |   yes    | `"power(1.6)"` | `"linear"`, `"power(γ)"`, `"cubic-bezier(x1,y1,x2,y2)"`; `γ` must be finite and `> 0`         |
| `outputFormat` | `string` |   yes    | same as input  | `"png"` / `"jpeg"` / `"jpg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"`                        |
| `advanced`     | `object` |   yes    | &mdash;        | See [Advanced Options](#advanced-options)                                                     |

### Advanced Options

<details>
<summary>Low-level pyramid configuration (usually unnecessary)</summary>

<br>

`advanced.mode: "auto"` derives defaults from `quality`, `curve`, the active blur span, image size, and `maxSigma`.
If you also provide other `advanced.*` fields, they still override those defaults.

| Field                           | Type     | Default  | Description                                         |
| :------------------------------ | :------- | :------- | :-------------------------------------------------- |
| `advanced.mode`                 | `string` | `"auto"` | `"auto"` or `"manual"`                              |
| `advanced.steps`                | `number` | derived  | Discrete blur levels, must be `>= 2`                |
| `advanced.maxLevels`            | `number` | derived  | Max downsampling depth, must be `>= 1`              |
| `advanced.targetLocalSigma`     | `number` | derived  | Per-level target local sigma, must be `> 0`         |
| `advanced.minLocalSigma`        | `number` | derived  | Per-level min local sigma, must be `> 0`            |
| `advanced.maxLocalSigma`        | `number` | derived  | Per-level max local sigma, must be `> 0`            |
| `advanced.downsampleStageSigma` | `number` | `0.5`    | Equivalent sigma after 2x downsample, must be `> 0` |

</details>

## Benchmark

```bash
cargo run -p variable_blur_bench -r -- --image docs/benchmark.jpg --warmup 5 --runs 20
```

```
Machine       : Windows 11 Pro | AMD Ryzen 9 9950X3D 16-Core Processor | 16C / 32T
Image         : 2400x1300 | Jpeg | Rgb8 | 593.59 KiB
Benchmark     : 5 warmup | 20 measured
Direction     : [1.0000, 0.0000] | start 0.0000 | end 2400.0000
Max sigma     : 32.00

Quality             avg     median        p95        min        max     MPix/s
q=0.00         65.08 ms   64.67 ms   67.08 ms   62.83 ms   67.87 ms      47.94
q=0.50         91.11 ms   90.78 ms   92.58 ms   88.62 ms   93.24 ms      34.25
q=1.00        541.93 ms  541.43 ms  546.71 ms  536.32 ms  549.56 ms       5.76
```

## License

Apache-2.0
