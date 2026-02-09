# Standard Library: FFT & Signal Domain

## Module

```aivi
module aivi.std.signal = {
  export domain Signal
  export Signal, Spectrum
  export fft, ifft, windowHann, normalize
}
```

## Types

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