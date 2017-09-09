using Imageflow.Bindings;

namespace Imageflow.Fluent
{
    public struct AnyColor
    {
        private ColorKind kind;
        private SrgbColor srgb;
        public static AnyColor Black => new AnyColor {kind = ColorKind.Black};
        public static AnyColor Transparent => new AnyColor {kind = ColorKind.Transparent};
        public static AnyColor FromHexSrgb(string hex) => new AnyColor {kind = ColorKind.Srgb, srgb = SrgbColor.FromHex(hex)};
        public static AnyColor Srgb(SrgbColor c) => new AnyColor {kind = ColorKind.Srgb, srgb = c};

        public object ToImageflowDynamic()
        {
            switch (kind)
            {
                case ColorKind.Black: return new {black = new { }};
                case ColorKind.Transparent: return new {transparent = new { }};
                case ColorKind.Srgb: return new {srgb = new { hex = srgb.ToHexUnprefixed()}};
                default: throw new ImageflowAssertionFailed("default");
            }
        }
    }
}