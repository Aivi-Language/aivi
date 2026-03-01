# FFT & Signal Domain

<!-- quick-info: {"kind":"module","name":"aivi.signal"} -->
The `Signal` domain provides tools for **Digital Signal Processing** (DSP), including the Fast Fourier Transform.

Signals are everything: audio from a mic, vibrations in a bridge, or stock market prices.
*   **Time Domain**: "How loud is it right now?"
*   **Frequency Domain**: "What notes are being played?"

The **Fast Fourier Transform (FFT)** is a legendary algorithm that converts Time into Frequency. It lets you unbake a cake to find the ingredients. If you want to filter noise from audio, analyze heartbeats, or compress images, you need this domain.

<!-- /quick-info -->
<div class="import-badge">use aivi.signal<span class="domain-badge">domain</span></div>

## Overview

<<< ../../snippets/from_md/stdlib/math/signal/overview.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/signal/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/signal/domain_definition.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **fft** signal<br><code>Signal -> Spectrum</code> | Transforms a signal into a frequency-domain spectrum. |
| **ifft** spectrum<br><code>Spectrum -> Signal</code> | Reconstructs a time-domain signal from its spectrum. |
| **windowHann** signal<br><code>Signal -> Signal</code> | Applies a Hann window to reduce spectral leakage. |
| **normalize** signal<br><code>Signal -> Signal</code> | Scales samples so the max absolute value is `1.0`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/signal/usage_examples.aivi{aivi}
