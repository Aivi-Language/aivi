# FFT & Signal Domain

<!-- quick-info: {"kind":"module","name":"aivi.signal"} -->
The `Signal` domain provides building blocks for digital signal processing, including transforms between time-domain and frequency-domain representations.
Use it for audio analysis, filtering, vibration data, sensor streams, or any other sequence of samples where the frequency content matters.
<!-- /quick-info -->
<div class="import-badge">use aivi.signal<span class="domain-badge">domain</span></div>

A simple way to think about this module: the time domain tells you what happens sample by sample, and the frequency domain tells you which repeating patterns are present.

## Overview

<<< ../../snippets/from_md/stdlib/math/signal/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/signal/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/signal/domain_definition.aivi{aivi}

## Core helpers

| Function | What it does |
| --- | --- |
| **fft** signal<br><code>Signal -> Spectrum</code> | Converts a time-domain signal into a frequency-domain spectrum. |
| **ifft** spectrum<br><code>Spectrum -> Signal</code> | Reconstructs a time-domain signal from a spectrum. |
| **windowHann** signal<br><code>Signal -> Signal</code> | Applies a Hann window to reduce spectral leakage before an FFT. |
| **normalize** signal<br><code>Signal -> Signal</code> | Scales samples so the maximum absolute value becomes `1.0`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/signal/usage_examples.aivi{aivi}
