# FFT & Signal Domain

Tools for **Digital Signal Processing** (DSP), including the Fast Fourier Transform.

## What is this?

Signals are everywhere: audio recorded from a microphone, vibrations from a sensor, or radio waves.
*   **Time Domain**: The signal as it happens (loudness over time).
*   **Frequency Domain**: The ingredients of the signal (how much bass vs. treble).

The **Fast Fourier Transform (FFT)** is a legendary algorithm that converts a Time signal into a Frequency signal. It lets you see the "notes" inside a chord.

## Why this exists

If you want to filter out background noise from audio, compress an image (JPEG), or analyze stock market cycles, you need DSP. This domain provides optimized algorithms so you don't have to write complex math loops yourself.

## Overview

```aivi
import aivi.std.math.signal use { fft, ifft }

// A simple signal (e.g., audio samples)
let timeDomain = [1.0, 0.5, 0.25, 0.125]

// Convert to frequencies to analyze pitch
let freqDomain = fft(timeDomain)
```

## Features

```aivi
Signal = { samples: List Float, rate: Float }
Spectrum = { bins: List Complex, rate: Float }
```

## Domain Definition

```aivi
domain Signal over Signal = {
  (+) : Signal -> Signal -> Signal
  (+) a b = { samples: zipWith (+) a.samples b.samples, rate: a.rate }
  
  (*) : Signal -> Float -> Signal
  (*) s k = { samples: map (\x -> x * k) s.samples, rate: s.rate }
}
```

## Helper Functions

```aivi
fft : Signal -> Spectrum
fft s = { bins: fftRaw s.samples, rate: s.rate }

ifft : Spectrum -> Signal
ifft s = { samples: ifftRaw s.bins, rate: s.rate }

windowHann : Signal -> Signal
windowHann s = { samples: applyHann s.samples, rate: s.rate }

normalize : Signal -> Signal
normalize s = s * (1.0 / maxAbs s.samples)
```

## Usage Examples

```aivi
use aivi.std.signal
use aivi.std.complex

audio = { samples: [0.0, 0.5, 1.0, 0.5], rate: 44100.0 }
spectrum = fft audio
recon = ifft spectrum
```