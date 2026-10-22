using System.Collections;
using System.Drawing;

namespace MicroTaskPager.GUI
{
    public enum DesktopMode
    {
        //Inherited,     // Takes the default mode
        Preview,        // Desktop -> Preview
        MicroTaskbar,   // Micro Traskbar
    }

    public enum BorderStyle
    {
        Box,
        Underline,
    }

    public class AppSettings
    {
        // DesktopModeDefault: Only changeable in the settings window, defines the default for new desktops
        // -> But is Desktop the right name? If it spans around multiple physical screens... yes, but it's still a desktop!
        public DesktopMode DesktopModeDefault { get; set; } = DesktopMode.Preview;
        public ArrayList DesktopModeArray { get; set; } = null;

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
                DesktopModeDefault = source.DesktopModeDefault,
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

