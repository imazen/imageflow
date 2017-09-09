namespace Imageflow.Fluent
{
// ReSharper disable InconsistentNaming
    public enum PixelFormat {
        Bgra_32 = 4,
        Bgr_32 = 70,
//        Bgr_24 = 3,
        //    Gray_8 = 1,
    }
        
    public enum ResampleWhen{
        Size_Differs,
        Size_Differs_Or_Sharpening_Requested,
        Always
    }
    // ReSharper enable InconsistentNaming
        
    public enum ScalingFloatspace {
        Srgb,
        Linear
    }
    public enum InterpolationFilter {
        RobidouxFast = 1,
        Robidoux = 2,
        RobidouxSharp = 3,
        Ginseng = 4,
        GinsengSharp = 5,
        Lanczos = 6,
        LanczosSharp = 7,
        Lanczos2 = 8,
        Lanczos2Sharp = 9,
        CubicFast = 10,
        Cubic = 11,
        CubicSharp = 12,
        CatmullRom = 13,
        Mitchell = 14,
    
        CubicBSpline = 15,
        Hermite = 16,
        Jinc = 17,
        RawLanczos3 = 18,
        RawLanczos3Sharp = 19,
        RawLanczos2 = 20,
        RawLanczos2Sharp = 21,
        Triangle = 22,
        Linear = 23,
        Box = 24,
        CatmullRomFast = 25,
        CatmullRomFastSharp = 26,
    
        Fastest = 27,
    
        MitchellFast = 28,
        NCubic = 29,
        NCubicSharp = 30,
    }


    public enum ColorKind
    {
        Black,
        Transparent,
        Srgb,
    }

    public enum PngBitDepth {
        Png_32,
        Png_24,
    }
}
