# FFT & Signal Domain

The `Signal` domain provides tools for **Digital Signal Processing** (DSP), including the Fast Fourier Transform.

Signals are everything: audio from a mic, vibrations in a bridge, or stock market prices.
*   **Time Domain**: "How loud is it right now?"
*   **Frequency Domain**: "What notes are being played?"

The **Fast Fourier Transform (FFT)** is a legendary algorithm that converts Time into Frequency. It lets you unbake a cake to find the ingredients. If you want to filter noise from audio, analyze heartbeats, or compress images, you need this domain.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/14_signal/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/14_signal/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/14_signal/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **fft** signal<br><pre><code>`Signal -> Spectrum`</code></pre> | Transforms a signal into a frequency-domain spectrum. |
| **ifft** spectrum<br><pre><code>`Spectrum -> Signal`</code></pre> | Reconstructs a time-domain signal from its spectrum. |
| **windowHann** signal<br><pre><code>`Signal -> Signal`</code></pre> | Applies a Hann window to reduce spectral leakage. |
| **normalize** signal<br><pre><code>`Signal -> Signal`</code></pre> | Scales samples so the max absolute value is `1.0`. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/14_signal/block_04.aivi{aivi}
