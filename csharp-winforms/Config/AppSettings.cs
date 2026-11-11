using System.Collections;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Documents;

namespace MicroTaskPager.Config
{
    public enum ViewMode
    {
        Preview,
        MicroTaskbarLight,
        MicroTaskbar,
    }

    public enum DesktopBorderStyle
    {
        Box,
        Underline,
    }

    // Defines which window gets activated after the pager switched to another desktop
    public enum DesktopChangeActivationMode
    {
        LastActiveWindow,
        WindowUnderCursor,
        None,
    }

    public class AppSettings
    {
        // ViewModeDefault: Only changeable in the settings window, defines the default for newly created desktop's screens
        public ViewMode ViewModeDefault { get; set; } = ViewMode.MicroTaskbarLight;
        public List<ViewMode> RendererViewModes { get; set; } = new List<ViewMode>();

        public int PagerHeight { get; set; } = 40;
        public int IconSize { get; set; } = 16;
        public int IconPaddingX { get; set; } = 0;
        public int IconPaddingY { get; set; } = 0;
        public bool ShowMinimizedWindowsLast { get; set; } = false;

        public double BackgroundOpacity { get; set; } = 1.0;

        public DesktopBorderStyle DesktopBorderStyle { get; set; } = DesktopBorderStyle.Box;
        public DesktopChangeActivationMode DesktopChangeActivation { get; set; } = DesktopChangeActivationMode.LastActiveWindow;
        public Color DesktopBackgroundColor { get; set; } = Color.FromArgb(64, 64, 64);
        public Color DesktopBorderColor { get; set; } = Color.FromArgb(32, 32, 32);
        public Color ActiveDesktopBorderColor { get; set; } = Color.LightBlue;
        public Color WindowBackgroundColor { get; set; } = Color.FromArgb(255, Color.Gray);
        public Color ActiveWindowBackgroundColor { get; set; } = Color.FromArgb(255, Color.Silver);
        public Color WindowBorderColor { get; set; } = Color.Silver;
        public Color ActiveWindowBorderColor { get; set; } = Color.White;

        public int PrimaryUpdateDelay { get; set; } = 200;
        public int SecondaryUpdateDelay { get; set; } = 500;
        public bool ShowAppInTaskbar { get; set; } = false;
        public bool DisableAlwaysOnTop { get; set; } = false;
        public bool AnimatedDesktopSwitch { get; set; } = false;

        public static AppSettings Clone(AppSettings source)
        {
            return new AppSettings
            {
                ViewModeDefault = source.ViewModeDefault,
                RendererViewModes = source.RendererViewModes == null ? null : new List<ViewMode>(source.RendererViewModes),
                PagerHeight = source.PagerHeight,
                IconSize = source.IconSize,
                IconPaddingX = source.IconPaddingX,
                IconPaddingY = source.IconPaddingY,
                ShowMinimizedWindowsLast = source.ShowMinimizedWindowsLast,

                BackgroundOpacity = source.BackgroundOpacity,
                DesktopBorderStyle = source.DesktopBorderStyle,
                DesktopChangeActivation = source.DesktopChangeActivation,
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
                DisableAlwaysOnTop = source.DisableAlwaysOnTop,
                AnimatedDesktopSwitch = source.AnimatedDesktopSwitch,
            };
        }
    }
}