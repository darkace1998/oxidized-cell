//! Example demonstrating Phase 3 audio features
//!
//! This example showcases the new audio capabilities:
//! - S/PDIF output
//! - Multi-channel audio (5.1, 7.1)
//! - Audio resampling
//! - Time stretching
//! - Audio codec support

use oc_audio::{
    codec::{AudioCodec, AudioDecoder, CodecConfig, PcmDecoder},
    mixer::{AudioMixer, ChannelLayout},
    resampler::{AudioResampler, ResamplerQuality},
    spdif::SpdifOutput,
    time_stretch::{AudioTimeStretcher, StretchAlgorithm, TimeStretchConfig},
};

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Phase 3 Audio Features Demo ===\n");

    // Demo 1: S/PDIF Output
    demo_spdif_output();

    // Demo 2: Multi-channel Audio
    demo_multichannel_audio();

    // Demo 3: Audio Resampling
    demo_audio_resampling();

    // Demo 4: Time Stretching
    demo_time_stretching();

    // Demo 5: Audio Codecs
    demo_audio_codecs();

    println!("\n=== All Phase 3 features demonstrated successfully! ===");
}

fn demo_spdif_output() {
    println!("--- Demo 1: S/PDIF Output ---");

    let mut spdif = SpdifOutput::new();

    // Enable S/PDIF
    spdif.enable().expect("Failed to enable S/PDIF");
    println!("✓ S/PDIF enabled at {} Hz", spdif.config().sample_rate);

    // Start S/PDIF output
    spdif.start().expect("Failed to start S/PDIF");
    println!("✓ S/PDIF output started");

    // Write some test samples (stereo)
    let test_samples: Vec<f32> = (0..1000)
        .map(|i| {
            let t = i as f32 / 48000.0;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();

    spdif
        .write_samples(&test_samples)
        .expect("Failed to write samples");
    println!(
        "✓ Written {} samples to S/PDIF buffer",
        test_samples.len()
    );

    // Stop S/PDIF
    spdif.stop().expect("Failed to stop S/PDIF");
    println!("✓ S/PDIF output stopped\n");
}

fn demo_multichannel_audio() {
    println!("--- Demo 2: Multi-channel Audio ---");

    // Test different channel layouts
    let layouts = vec![
        (ChannelLayout::Mono, "Mono (1 channel)"),
        (ChannelLayout::Stereo, "Stereo (2 channels)"),
        (ChannelLayout::Surround51, "5.1 Surround (6 channels)"),
        (ChannelLayout::Surround71, "7.1 Surround (8 channels)"),
    ];

    for (layout, name) in layouts {
        let mut mixer = AudioMixer::new(layout);
        let source_id = mixer.add_source(layout);

        // Generate test samples for this layout
        let num_samples = layout.num_channels() * 10; // 10 frames
        let test_samples: Vec<f32> = (0..num_samples).map(|i| (i % 2) as f32 * 0.5).collect();

        mixer
            .write_to_source(source_id, &test_samples)
            .expect("Failed to write samples");

        println!("✓ {} - {} channels configured", name, layout.num_channels());

        // Mix the audio
        let mut output = vec![0.0; num_samples];
        mixer.mix(&mut output, 10);
        println!("  Mixed {} samples", output.len());
    }

    println!();
}

fn demo_audio_resampling() {
    println!("--- Demo 3: Audio Resampling ---");

    // Test resampling from 44.1kHz to 48kHz
    let qualities = vec![
        (ResamplerQuality::Low, "Linear"),
        (ResamplerQuality::Medium, "Cubic"),
        (ResamplerQuality::High, "Sinc"),
    ];

    for (quality, name) in qualities {
        let mut resampler = AudioResampler::with_quality(44100, 48000, 2, quality);

        println!("  {} interpolation:", name);
        println!("    Input rate:  {} Hz", resampler.input_rate());
        println!("    Output rate: {} Hz", resampler.output_rate());
        println!("    Ratio:       {:.4}", resampler.ratio());

        // Generate test input (stereo sine wave at 44.1kHz)
        let input: Vec<f32> = (0..882)
            .map(|i| {
                let t = i as f32 / 44100.0;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
            })
            .collect();

        let mut output = Vec::new();
        resampler
            .resample(&input, &mut output)
            .expect("Failed to resample");

        println!("    Resampled {} → {} samples", input.len(), output.len());
    }

    println!("✓ Audio resampling demonstrated\n");
}

fn demo_time_stretching() {
    println!("--- Demo 4: Time Stretching ---");

    let algorithms = vec![
        (StretchAlgorithm::Simple, "Overlap-Add"),
        (StretchAlgorithm::PhaseVocoder, "Phase Vocoder"),
        (StretchAlgorithm::Wsola, "WSOLA"),
    ];

    for (algorithm, name) in algorithms {
        let config = TimeStretchConfig {
            sample_rate: 48000,
            num_channels: 2,
            algorithm,
            window_size: 256,
            overlap: 64,
        };

        let mut stretcher = AudioTimeStretcher::new(config);

        println!("  {} algorithm:", name);

        // Generate test input
        let input: Vec<f32> = (0..2048)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
            })
            .collect();

        // Test different stretch factors
        for factor in &[0.8, 1.0, 1.2, 1.5] {
            let mut output = Vec::new();
            stretcher.reset();
            stretcher
                .stretch(&input, *factor, &mut output)
                .expect("Failed to stretch");

            println!("    Factor {:.1}x: {} → {} samples", factor, input.len(), output.len());
        }
    }

    println!("✓ Time stretching demonstrated\n");
}

fn demo_audio_codecs() {
    println!("--- Demo 5: Audio Codecs ---");

    // Test PCM decoder
    let mut pcm_decoder = PcmDecoder::new();
    let config = CodecConfig {
        codec: AudioCodec::Pcm,
        sample_rate: 48000,
        num_channels: 2,
        bit_rate: None,
        bits_per_sample: Some(16),
    };

    pcm_decoder.init(config).expect("Failed to init PCM decoder");
    println!("✓ PCM decoder initialized");

    // Test 16-bit PCM decoding
    let pcm_data: Vec<u8> = vec![
        0x00, 0x00, // Sample 1 (0)
        0xFF, 0x7F, // Sample 2 (32767)
        0x00, 0x80, // Sample 3 (-32768)
        0x00, 0x00, // Sample 4 (0)
    ];

    let mut decoded = Vec::new();
    let sample_count = pcm_decoder
        .decode(&pcm_data, &mut decoded)
        .expect("Failed to decode");

    println!("  Decoded {} samples from PCM data", sample_count);
    println!("  Sample values: {:?}", &decoded[..4.min(decoded.len())]);

    // Show support for other codecs
    println!("\n  Supported codecs:");
    let codecs = vec![
        AudioCodec::Pcm,
        AudioCodec::Lpcm,
        AudioCodec::Aac,
        AudioCodec::At3,
        AudioCodec::At3Plus,
        AudioCodec::Mp3,
        AudioCodec::Ac3,
        AudioCodec::Dts,
    ];

    for codec in codecs {
        let compressed: &str = if codec.is_compressed() { "Compressed" } else { "Uncompressed" };
        let multichannel: &str = if codec.supports_multichannel() {
            "Multi-channel"
        } else {
            "Stereo"
        };
        println!("    • {} - {} - {}", codec.name(), compressed, multichannel);
    }

    println!("✓ Audio codec support demonstrated\n");
}
