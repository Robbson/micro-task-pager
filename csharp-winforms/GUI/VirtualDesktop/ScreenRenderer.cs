using System;
using System.Collections.Generic;
using System.ComponentModel.Design;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Forms;
using System.Windows.Media.Media3D;
using MicroTaskPager.Config;
using MicroTaskPager.Core;
using MicroTaskPager.Core.API;

namespace MicroTaskPager.GUI.VirtualDesktop
{
    // It's the pure content renderer, depending on the used mode, also window order is determined here
    public class ScreenRenderer
    {
        // Shared by all renderers and for the whole process lifetime, because a Font is a native GDI object.
        // Creating one per renderer (and per settings preview) leaked a handle each time.
        private static readonly Font _font =
            new Font("Segoe UI Symbol", Dpi.Scale(12), FontStyle.Regular, GraphicsUnit.Pixel);

        // A translucent black overlay dims both the tile background and the icon without
        // changing the configured window colors.
        private static readonly Color _minimizedOverlayColor = Color.FromArgb(120, 0, 0, 0);

        public Size Size { get; set; }
        public MainForm Form { get; set; }
        public Point Location { get; set; }
        public Screen AttachedScreen { get; set; }
        public Desktop VirtualDesktop { get; set; }
        public Dictionary<IntPtr, Rectangle> WindowAreas { get; set; } = new Dictionary<IntPtr, Rectangle>();
        public ViewMode Mode { get; set; }

        private Point MousePosition { get => System.Windows.Forms.Control.MousePosition; }
        private Window ActiveWindow { get => Form.Windows.SingleOrDefault(x => x.IsActive); }

        public void Render(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            WindowAreas.Clear();

            // Only windows that belong to this renderer's monitor take part in the mode. Counting
            // windows from other monitors here would mis-center and mis-wrap the Micro Taskbar tiles.
            // Minimized windows have no useful screen geometry, so they are only offered to the
            // Micro Taskbar view and only when the user enabled them.
            var candidateWindows = Form.Windows.Where(x =>
                x.VirtualDesktopIndex == VirtualDesktop.VirtualDesktopIndex
                && (
                    (Mode == ViewMode.MicroTaskbarLight) || !x.IsMinimized)
                && Screen.FromHandle(x.Handle).DeviceName == AttachedScreen.DeviceName);

            Window[] windows;
            if (Mode == ViewMode.Preview)
            {
                // For preview view mode, all windows have to be sorted in z-order to draw from back to front
                windows = candidateWindows.OrderBy(x => x.ZOrder).ToArray();
            }
            else if (Form.Options.ShowWhenMinimized == MinimizedWindowVisibility.AfterOthers)
            {
                // Keep the normal order but append minimized windows after all others.
                windows = candidateWindows
                    .OrderBy(x => x.IsMinimized ? 1 : 0)
                    .ThenBy(x => x.ProcessID)
                    .ThenBy(x => x.Handle.ToInt64())
                    .ToArray();
            }
            else
            {
                // Otherwise order should stay the same (using process id and handle)
                // -> Order by process id is similar to order in taskbar, as long as the user doesn't move them.
                windows = candidateWindows
                    .OrderBy(x => x.ProcessID)
                    .ThenBy(x => x.Handle.ToInt64())
                    .ToArray();
            }

            if (windows.Length == 0) return;

            switch (Mode)
            {
                case ViewMode.MicroTaskbar:
                    RenderMicroTaskbar(g, windows);
                    break;
                default:
                    RenderPreview(g, windows);
                    break;
            }
        }

        private static void DrawIcon(Graphics g, Bitmap icon, Rectangle destination)
        {
            if (icon == null)
            {
                return;
            }

            // Without an explicit high quality mode, GDI+ resamples the large shell icons of
            // packaged apps with visible aliasing. Exact-size draws stay crisp.
            if (icon.Width == destination.Width && icon.Height == destination.Height)
            {
                g.DrawImageUnscaled(icon, destination.Location);
                return;
            }

            InterpolationMode interpolationMode = g.InterpolationMode;
            PixelOffsetMode pixelOffsetMode = g.PixelOffsetMode;
            try
            {
                g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                g.DrawImage(icon, destination);
            }
            finally
            {
                g.InterpolationMode = interpolationMode;
                g.PixelOffsetMode = pixelOffsetMode;
            }
        }

        private void RenderPreview(Graphics g, Window[] windows)
        {
            int iconSize = Dpi.Scale(Form.Options.IconSize);

            foreach (var wnd in windows)
            {
                // --- Calculating ---

                var x = wnd.Dimensions.X;
                var y = wnd.Dimensions.Y;
                x -= AttachedScreen.Bounds.Left;
                y -= AttachedScreen.Bounds.Top;
                var area = new Rectangle(
                    x, y, wnd.Dimensions.Width - Form.BorderSize, wnd.Dimensions.Height - Form.BorderSize);

                // Scale rectangles down to the thumbnail's desired size
                var ar = Helpers.AspectRatioResize(
                    new Size(AttachedScreen.Bounds.Width, AttachedScreen.Bounds.Height), 0, Form.PagerHeightPixels);
                float percentageWidth = (float)ar.Width * 100 / AttachedScreen.Bounds.Width;
                float percentageHeight = (float)ar.Height * 100 / AttachedScreen.Bounds.Height;

                area.X = (int)(area.X * (percentageWidth / 100));
                area.Y = (int)(area.Y * (percentageHeight / 100));
                area.Width = (int)(area.Width * (percentageWidth / 100));
                area.Height = (int)(area.Height * (percentageWidth / 100));

                area.X += Location.X;
                area.Y += Location.Y;
                WindowAreas[wnd.Handle] = area;

                // --- Drawing ---

                Color fillColor = wnd.IsActive ? Form.Options.ActiveWindowBackgroundColor : Form.Options.WindowBackgroundColor;
                Color borderColor = wnd.IsActive ? Form.Options.ActiveWindowBorderColor : Form.Options.WindowBorderColor;

                // Window rectangle fill
                using (var fillBrush = new SolidBrush(fillColor))
                {
                    g.FillRectangle(
                        fillBrush,
                        new Rectangle(area.X, area.Y, area.Width - Form.BorderSize, area.Height - Form.BorderSize));
                }

                // Window icon
                var state = g.Save();
                try
                {
                    g.SetClip(area);
                    DrawIcon(g, wnd.Icon, new Rectangle(
                        (area.X + area.Width / 2) - iconSize / 2,
                        (area.Y + area.Height / 2) - iconSize / 2,
                        iconSize,
                        iconSize));
                }
                finally
                {
                    g.Restore(state);
                }

                // Window border
                // GDI+ centers the pen stroke on the rectangle path, so inset by half the border
                // width to keep the complete border inside the window area.
                int borderInset = Form.BorderSize / 2;
                using (var borderBrush = new SolidBrush(borderColor))
                using (var borderPen = new Pen(borderBrush, Form.BorderSize))
                {
                    g.DrawRectangle(
                        borderPen,
                        new Rectangle(
                            area.X + borderInset,
                            area.Y + borderInset,
                            area.Width - Form.BorderSize,
                            area.Height - Form.BorderSize));
                }
            }
        }

        private void RenderMicroTaskbar(Graphics g, Window[] windows)
        {
            int iconsNum = windows.Length;
            int iconSize = Dpi.Scale(Form.Options.IconSize);
            int iconPaddingX = Dpi.Scale(Form.Options.IconPaddingX);
            int iconPaddingY = Dpi.Scale(Form.Options.IconPaddingY);
            int width = Size.Width;
            int height = Size.Height;

            // An icon can have user specified padding, together it's a tile
            int tileWidth = iconSize + iconPaddingX;
            int tileHeight = iconSize + iconPaddingY;

            // Calculate the available amount of columns and rows and if we have enough for all tiles
            int remainingIcons = 0; // for which we have no space
            int colsNum = width / tileWidth; // tiles per row
            int rowsNum = height / tileHeight;
            if (rowsNum == 0) rowsNum = 1; // at least one row should always be possible, even when very tight and icons gettings cropped

            int screenCenterX, screenCenterY;
            if (iconsNum > colsNum)
            {
                int requiredRows = iconsNum / colsNum;
                remainingIcons = iconsNum % colsNum;
                if (remainingIcons > 0) requiredRows++;
                if (requiredRows > rowsNum)
                {
                    remainingIcons = iconsNum - colsNum * rowsNum;
                    screenCenterY = (height % tileHeight) / 2;
                }
                else
                {
                    screenCenterY = (height % (tileHeight * requiredRows)) / 2;
                }

                screenCenterX = (width % tileWidth) / 2;
            }
            else
            {
                screenCenterX = (width - (tileWidth * iconsNum)) / 2;
                screenCenterY = (height - tileHeight) / 2;
            }

            // Center all tiles
            int rootX = Location.X + screenCenterX;
            int rootY = Location.Y + screenCenterY;

            // Position and draw all tiles
            int col = 0, row = 0;
            foreach (var wnd in windows)
            {
                // Calc tile position
                int yPos = row * tileHeight;
                int xPos = col * tileWidth;

                // Calc selection area rectangle and the center position for the icon in it
                var selectionArea = new Rectangle(rootX + xPos, rootY + yPos, tileWidth, tileHeight);
                var areaCenterOffsetX = selectionArea.X + selectionArea.Width / 2 - iconSize / 2;
                var areaCenterOffsetY = selectionArea.Y + selectionArea.Height / 2 - iconSize / 2;

                // --- Drawing ---
                if (remainingIcons > 0 && col == colsNum - 1 && row == rowsNum - 1)
                {
                    // Reserve the last possible position for the remaining count (+1 because it reduces the available slots)
                    using (var textBrush = new SolidBrush(Form.Options.ActiveWindowBorderColor))
                    {
                        g.DrawString("+" + (remainingIcons + 1).ToString(), _font, textBrush, areaCenterOffsetX, areaCenterOffsetY - 1);
                    }

                    break; // No more drawing
                }
                else
                {
                    // Highlight the selection area
                    if (wnd.IsActive)
                    {
                        using (var highlightBrush = new SolidBrush(Form.Options.ActiveWindowBackgroundColor))
                        {
                            g.FillRectangle(highlightBrush, selectionArea);
                        }
                    }

                    // Place the icon in the selection area's center
                    DrawIcon(g, wnd.Icon, new Rectangle(areaCenterOffsetX, areaCenterOffsetY, iconSize, iconSize));

                    // Dim minimized windows so they can be told apart from normal ones
                    if (wnd.IsMinimized)
                    {
                        using (var overlayBrush = new SolidBrush(_minimizedOverlayColor))
                        {
                            g.FillRectangle(overlayBrush, selectionArea);
                        }
                    }

                    // Make the window selectable
                    WindowAreas[wnd.Handle] = selectionArea;
                }

                if (col < colsNum - 1)
                {
                    col++;
                }
                else
                {
                    row++;
                    col = 0;
                }
            }
        }
    }
}
