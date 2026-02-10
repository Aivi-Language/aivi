# FFT & Signal Domain

The `Signal` domain provides tools for **Digital Signal Processing** (DSP), including the Fast Fourier Transform.

Signals are everything: audio from a mic, vibrations in a bridge, or stock market prices.
*   **Time Domain**: "How loud is it right now?"
*   **Frequency Domain**: "What notes are being played?"

The **Fast Fourier Transform (FFT)** is a legendary algorithm that converts Time into Frequency. It lets you unbake a cake to find the ingredients. If you want to filter noise from audio, analyze heartbeats, or compress images, you need this domain.

## Overview

```aivi
use aivi.std.math.signal (fft, ifft)

// A simple signal (e.g., audio samples)
timeDomain = [1.0, 0.5, 0.25, 0.125]

// Convert to frequencies to analyze pitch
freqDomain = fft(timeDomain)
```

## Performance and Implementation (Rust)

FFT and DSP must be implemented using established, optimized crates. Prefer `rustfft` (and `realfft` where needed) for core transforms, and avoid custom FFT kernels.

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
use aivi.std.number.complex

audio = { samples: [0.0, 0.5, 1.0, 0.5], rate: 44100.0 }
spectrum = fft audio
recon = ifft spectrum
```
