using System;
using System.Drawing;
using System.Windows.Forms;

namespace MicroTaskPager.GUI
{
    public class ThemedGroupBox : GroupBox
    {
        private Font _captionFont;

        public ThemedGroupBox()
        {
            SetStyle(
                ControlStyles.UserPaint
                | ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.ResizeRedraw,
                true);

            UpdateCaptionFont();
        }

        protected override void OnFontChanged(EventArgs e)
        {
            base.OnFontChanged(e);
            UpdateCaptionFont();
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics graphics = e.Graphics;
            Rectangle client = ClientRectangle;

            using (var background = new SolidBrush(BackColor))
            {
                graphics.FillRectangle(background, client);
            }

            int lineY = _captionFont.Height / 2;
            Color borderColor = SystemInformation.HighContrast
                ? SystemColors.WindowText
                : SystemColors.ControlDarkDark;
            using (var border = new Pen(borderColor))
            {
                graphics.DrawRectangle(
                    border,
                    client.Left,
                    lineY,
                    client.Width - 1,
                    client.Height - lineY - 1);
            }

            Size captionSize = TextRenderer.MeasureText(Text, _captionFont);
            var captionBounds = new Rectangle(8, 0, captionSize.Width + 4, _captionFont.Height);
            using (var captionBackground = new SolidBrush(BackColor))
            {
                graphics.FillRectangle(captionBackground, captionBounds);
            }

            TextRenderer.DrawText(
                graphics,
                Text,
                _captionFont,
                captionBounds,
                SystemInformation.HighContrast ? SystemColors.WindowText : SystemColors.ControlText,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _captionFont?.Dispose();
            }

            base.Dispose(disposing);
        }

        private void UpdateCaptionFont()
        {
            _captionFont?.Dispose();
            _captionFont = new Font(Font, FontStyle.Bold);
        }
    }
}
