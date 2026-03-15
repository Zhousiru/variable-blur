<div align="center">

# variable-blur

**Apple-style variable blur for Node.js**

<br>

<img src="docs/example-blurred.png" width="720">

<br>
<br>

[中文文档](./README.zh-CN.md)

</div>

## Installation

```bash
npm install variable-blur
```

Prebuilt binaries for macOS / Linux / Windows (x64 & arm64).

## Quick Start

```js
import sharp from 'sharp'
import { variableBlur } from 'variable-blur'

const input = await sharp('photo.jpg').png().toBuffer()

const output = variableBlur({
  buffer: input,
  options: {
    preset: 'balanced',
    maxSigma: 32,
  },
})

await sharp(output).toFile('photo-blurred.png')
```

## Debug UI

```bash
cargo run -p debug-ui --release
```

Built-in [egui](https://github.com/emilk/egui) tool for real-time parameter tuning.

## API

### `variableBlur(input): Buffer`

| Parameter       | Type      | Description                           |
| :-------------- | :-------- | :------------------------------------ |
| `input.buffer`  | `Buffer`  | Encoded image (PNG, JPEG, WebP, etc.) |
| `input.options` | `object?` | See below                             |

### Options

| Field          | Type     | Optional | Default       | Description                                                  |
| :------------- | :------- | :------: | :------------ | :----------------------------------------------------------- |
| `x`            | `number` |   yes    | `1`           | X component of blur direction                                |
| `y`            | `number` |   yes    | `0`           | Y component of blur direction                                |
| `start`        | `number` |   yes    | auto          | Projection coordinate where blur begins                      |
| `end`          | `number` |   yes    | auto          | Projection coordinate where blur reaches max                 |
| `preset`       | `string` |   yes    | `"balanced"`  | `"fast"` / `"balanced"` / `"high"`                           |
| `maxSigma`     | `number` |   yes    | preset        | Max Gaussian sigma (`fast`=24, `balanced`=32, `high`=40)     |
| `curve`        | `string` |   yes    | `"power"`     | `"linear"`, `"power(γ)"`, `"cubic-bezier(x1,y1,x2,y2)"`      |
| `schedule`     | `string` |   yes    | `"power"`     | `"linear"`, `"power(γ)"`                                     |
| `outputFormat` | `string` |   yes    | same as input | `"png"` / `"jpeg"` / `"webp"` / `"bmp"` / `"tiff"` / `"tga"` |
| `advanced`     | `object` |   yes    | &mdash;       | See [Advanced Options](#advanced-options)                    |

### Advanced Options

<details>
<summary>Low-level pyramid configuration (usually unnecessary)</summary>

<br>

| Field                           | Type     | Default  | Description                           |
| :------------------------------ | :------- | :------- | :------------------------------------ |
| `advanced.mode`                 | `string` | `"auto"` | `"auto"` or `"manual"`                |
| `advanced.steps`                | `number` | preset   | Discrete blur levels                  |
| `advanced.maxLevels`            | `number` | preset   | Max downsampling depth                |
| `advanced.targetLocalSigma`     | `number` | preset   | Per-level target local sigma          |
| `advanced.minLocalSigma`        | `number` | preset   | Per-level min local sigma             |
| `advanced.maxLocalSigma`        | `number` | preset   | Per-level max local sigma             |
| `advanced.downsampleStageSigma` | `number` | `0.5`    | Equivalent sigma before 2x downsample |

| Preset     | steps | maxLevels | targetLocalSigma | minLocalSigma | maxLocalSigma |
| :--------- | :---: | :-------: | :--------------: | :-----------: | :-----------: |
| `fast`     |   7   |     6     |       1.6        |      0.3      |      3.0      |
| `balanced` |  10   |     4     |       2.0        |      0.5      |      4.0      |
| `high`     |  14   |     2     |       2.4        |      0.8      |      5.0      |

</details>

## License

MIT
