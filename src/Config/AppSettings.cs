using System.Collections;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Documents;

namespace MicroTaskPager.Config
{
    public enum ViewMode
    {
        Preview,
        MicroTaskbar,
    }

    public enum BorderStyle
    {
        Box,
        Underline,
    }

    public class AppSettings
    {
        // ViewModeDefault: Only changeable in the settings window, defines the default for newly created desktop's screens
        public ViewMode ViewModeDefault { get; set; } = ViewMode.MicroTaskbar;
        public List<ViewMode> RendererViewModes { get; set; } = new List<ViewMode>();

        public int PagerHeight { get; set; } = 40;
        public int IconPaddingX { get; set; } = 0;
        public int IconPaddingY { get; set; } = 0;

        public double BackgroundOpacity { get; set; } = 1.0;

        public BorderStyle DesktopBorderStyle { get; set; } = BorderStyle.Box;
        public Color DesktopBackgroundColor { get; set; } = Color.FromArgb(64, 64, 64);
        public Color DesktopBorderColor { get; set; } = Color.FromArgb(32, 32, 32);
        public Color ActiveDesktopBorderColor { get; set; } = Color.LightBlue;
        public Color WindowBackgroundColor { get; set; } = Color.FromArgb(255, Color.Gray);
        public Color ActiveWindowBackgroundColor { get; set; } = Color.FromArgb(255, Color.Silver);
        public Color WindowBorderColor { get; set; } = Color.Silver;
        public Color ActiveWindowBorderColor { get; set; } = Color.White;

        public int PrimaryUpdateDelay { get; set; } = 200;
        public int SecondaryUpdateDelay { get; set; } = 500;
        public bool ShowAppInTaskbar { get; set; } = true;

        public static AppSettings Clone(AppSettings source)
        {
            return new AppSettings
            {
                ViewModeDefault = source.ViewModeDefault,
                RendererViewModes = source.RendererViewModes == null ? null : new List<ViewMode>(source.RendererViewModes),
                PagerHeight = source.PagerHeight,
                IconPaddingX = source.IconPaddingX,
                IconPaddingY = source.IconPaddingY,

                BackgroundOpacity = source.BackgroundOpacity,
                DesktopBorderStyle = source.DesktopBorderStyle,
                DesktopBackgroundColor = source.DesktopBackgroundColor,
                DesktopBorderColor = source.DesktopBorderColor,
                ActiveDesktopBorderColor = source.ActiveDesktopBorderColor,
                WindowBackgroundColor = source.WindowBackgroundColor,
                ActiveWindowBackgroundColor = source.ActiveWindowBackgroundColor,
                WindowBorderColor = source.WindowBorderColor,
                ActiveWindowBorderColor = source.ActiveWindowBorderColor,

                PrimaryUpdateDelay = source.PrimaryUpdateDelay,
                SecondaryUpdateDelay = source.SecondaryUpdateDelay,
                ShowAppInTaskbar = source.ShowAppInTaskbar,
            };
        }
    }
}