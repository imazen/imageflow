use imageflow_commands::*;

#[test]
fn roundtrip_basic_pipeline() {
    let req = BuildRequest {
        io: vec![
            IoObject { io_id: 0, direction: IoDirection::In, io: IoEnum::Placeholder },
            IoObject { io_id: 1, direction: IoDirection::Out, io: IoEnum::OutputBuffer },
        ],
        pipeline: Pipeline::Steps(vec![
            Step::Decode(DecodeStep { io_id: 0, color: None, hints: None, ultrahdr: None }),
            Step::Constrain(ConstrainStep {
                mode: ConstraintMode::Fit,
                w: Some(800),
                h: Some(600),
                gravity: None,
                background: None,
                hints: None,
            }),
            Step::Encode(EncodeStep {
                io_id: 1,
                format: Some(OutputFormat::Jpeg),
                quality: Some(QualityTarget::Quality(85.0)),
                color: None,
                ultrahdr: None,
                prefer_lossless_jpeg: false,
                hints: None,
                matte: None,
            }),
        ]),
        security: None,
    };

    let json = serde_json::to_string_pretty(&req).unwrap();
    let back: BuildRequest = serde_json::from_str(&json).unwrap();
    match &back.pipeline {
        Pipeline::Steps(steps) => assert_eq!(steps.len(), 3),
        _ => panic!("expected Steps"),
    }
}

#[test]
fn roundtrip_adjust_step() {
    let step = Step::Adjust(AdjustStep {
        exposure: 0.5,
        contrast: -0.2,
        saturation: 0.3,
        vibrance: 0.1,
        ..Default::default()
    });

    let json = serde_json::to_string(&step).unwrap();
    let back: Step = serde_json::from_str(&json).unwrap();
    match back {
        Step::Adjust(adj) => {
            assert!((adj.exposure - 0.5).abs() < 0.001);
            assert!((adj.contrast - (-0.2)).abs() < 0.001);
            assert_eq!(adj.brightness, 0.0); // default
        }
        _ => panic!("wrong step variant"),
    }
}

#[test]
fn roundtrip_quality_targets() {
    let cases: Vec<QualityTarget> = vec![
        QualityTarget::Quality(85.0),
        QualityTarget::MatchSource { tolerance: Some(0.3), shrink_guarantee: true },
        QualityTarget::Butteraugli(1.5),
        QualityTarget::Ssimulacra2(75.0),
        QualityTarget::Lossless,
    ];

    for qt in &cases {
        let json = serde_json::to_string(qt).unwrap();
        let back: QualityTarget = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn roundtrip_all_step_variants() {
    // Verify every Step variant can serialize and deserialize
    let steps: Vec<Step> = vec![
        Step::Decode(DecodeStep {
            io_id: 0,
            color: Some(ColorHandling {
                icc: IccHandling::ConvertToSrgb,
                profile_errors: ProfileErrorHandling::Ignore,
                honor_gama_chrm: HonorGamaChrm::Never,
            }),
            hints: Some(DecodeHints {
                jpeg_downscale: Some(DownscaleTarget { w: 400, h: 300 }),
                webp_downscale: None,
                frame: Some(2),
            }),
            ultrahdr: Some(UltraHdrDecodeMode::HdrReconstruct { boost: Some(2.0) }),
        }),
        Step::Encode(EncodeStep {
            io_id: 1,
            format: Some(OutputFormat::Jxl),
            quality: Some(QualityTarget::Butteraugli(1.0)),
            color: Some(OutputColor { profile: OutputProfile::DisplayP3 }),
            ultrahdr: None,
            prefer_lossless_jpeg: true,
            hints: Some(EncoderHints {
                jxl: Some(JxlHints { distance: Some(1.0), effort: Some(7) }),
                ..Default::default()
            }),
            matte: Some(Color::white()),
        }),
        Step::Constrain(ConstrainStep {
            mode: ConstraintMode::FitCrop,
            w: Some(800),
            h: Some(600),
            gravity: Some(Gravity::BottomRight),
            background: Some(Color::black()),
            hints: Some(ResizeHints {
                filter: Some(Filter::Lanczos),
                sharpen_percent: Some(15.0),
                scaling_colorspace: Some(ScalingColorspace::Linear),
                resample_when: None,
                sharpen_when: None,
            }),
        }),
        Step::Resize(ResizeStep { w: 1920, h: 1080, hints: None }),
        Step::Crop(CropStep { x1: 10, y1: 20, x2: 800, y2: 600 }),
        Step::CropWhitespace(CropWhitespaceStep { threshold: 80, percent_padding: 5.0 }),
        Step::Region(RegionStep {
            x1: -10.0,
            y1: -10.0,
            x2: 810.0,
            y2: 610.0,
            background: Some(Color::transparent()),
        }),
        Step::RegionPercent(RegionPercentStep {
            x1: 10.0,
            y1: 10.0,
            x2: 90.0,
            y2: 90.0,
            background: None,
        }),
        Step::Orient(OrientStep::Auto),
        Step::Orient(OrientStep::Exif(6)),
        Step::FlipH,
        Step::FlipV,
        Step::Rotate90,
        Step::Rotate180,
        Step::Rotate270,
        Step::Transpose,
        Step::ExpandCanvas(ExpandCanvasStep {
            left: 10,
            top: 20,
            right: 10,
            bottom: 20,
            color: Color::white(),
        }),
        Step::FillRect(FillRectStep {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 100,
            color: Color::Hex("#ff000080".into()),
        }),
        Step::CreateCanvas(CreateCanvasStep { w: 1920, h: 1080, color: Color::black() }),
        Step::RoundCorners(RoundCornersStep { mode: RoundCornersMode::Circle, background: None }),
        Step::Adjust(AdjustStep {
            exposure: 0.5,
            contrast: -0.2,
            highlights: 0.3,
            shadows: -0.1,
            vibrance: 0.2,
            saturation: 0.0,
            clarity: 0.4,
            temperature: 0.1,
            tint: -0.05,
            noise_reduction: 0.3,
            deblock: 0.0,
            brightness: 0.0,
        }),
        Step::ColorMatrix(ColorMatrixStep {
            matrix: [
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }),
        Step::ColorFilter(ColorFilterStep::Sepia),
        Step::ColorFilter(ColorFilterStep::Alpha(0.5)),
        Step::Sharpen(SharpenStep { amount: 25.0 }),
        Step::Blur(BlurStep { sigma: 2.0 }),
        Step::WhiteBalance(WhiteBalanceStep { threshold: Some(0.01) }),
        Step::DrawImage(DrawImageStep {
            io_id: Some(2),
            source: None,
            x: 100,
            y: 100,
            w: 200,
            h: 200,
            blend: BlendMode::Normal,
            hints: None,
        }),
        Step::Watermark(WatermarkStep {
            io_id: Some(3),
            source: None,
            fit_box: Some(FitBox { left: 0.8, top: 0.8, right: 0.95, bottom: 0.95 }),
            gravity: Gravity::BottomRight,
            opacity: 0.5,
            min_canvas_width: Some(400),
            min_canvas_height: None,
            hints: None,
        }),
        Step::CommandString(CommandStringStep {
            value: "w=800&h=600&mode=crop".into(),
            decode: Some(0),
            encode: Some(1),
        }),
    ];

    for step in &steps {
        let json = serde_json::to_string(step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2, "roundtrip failed for: {json}");
    }
}

#[test]
fn security_limits_roundtrip() {
    let limits = SecurityLimits {
        max_decode_size: Some(SizeLimit {
            w: Some(10000),
            h: Some(10000),
            megapixels: Some(100.0),
        }),
        max_frame_size: None,
        max_encode_size: None,
        process_timeout_ms: Some(30000),
        max_memory_bytes: Some(1024 * 1024 * 512),
        max_encoder_threads: Some(4),
    };

    let json = serde_json::to_string_pretty(&limits).unwrap();
    let back: SecurityLimits = serde_json::from_str(&json).unwrap();
    assert_eq!(back.process_timeout_ms, Some(30000));
    assert!(back.max_frame_size.is_none());
}
