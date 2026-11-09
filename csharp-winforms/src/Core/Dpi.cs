using System;
using System.Drawing;
using MicroTaskPager.Core.API;

namespace MicroTaskPager.Core
{
    // Central DPI scaling for the pixel based parts of the app (the pager and its custom rendering).
    // The process is declared system DPI aware in app.manifest, so all windows use the system DPI
    // and it does not change while the app is running.
    public static class Dpi
    {
        private const int DefaultDpi = 96;

        private static readonly double _scaleFactor = GetScaleFactor();

        public static double ScaleFactor => _scaleFactor;

        public static int Scale(int value) => (int)Math.Round(value * _scaleFactor, MidpointRounding.AwayFromZero);

        public static float Scale(float value) => (float)(value * _scaleFactor);

        public static Size Scale(Size size) => new Size(Scale(size.Width), Scale(size.Height));

        public static Point Scale(Point point) => new Point(Scale(point.X), Scale(point.Y));

        private static double GetScaleFactor()
        {
            uint dpi = 0;
            try
            {
                dpi = WinAPI.GetDpiForSystem();
            }
            catch (EntryPointNotFoundException)
            {
                // GetDpiForSystem is only available on Windows 10 1607 and later
            }

            if (dpi == 0)
            {
                using (var graphics = Graphics.FromHwnd(IntPtr.Zero))
                {
                    dpi = (uint)Math.Round(graphics.DpiX);
                }
            }

            return dpi == 0 ? 1.0 : dpi / (double)DefaultDpi;
        }
    }
}
