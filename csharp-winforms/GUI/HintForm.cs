using System;
using System.Drawing;
using System.Windows.Forms;
using MicroTaskPager.Core;

namespace MicroTaskPager.GUI
{
    // Small owner-drawn, non-activating popup. The pager skips its topmost re-assertion while
    // a hint is visible, so these windows stay above the pager without being owned forms.
    // Keeping them unowned avoids mutating Application.OpenForms while Application.Exit closes it.
    internal class HintForm : Form
    {
        private const int WsExNoActivate = 0x08000000;
        private const int WsExToolWindow = 0x00000080;
        private const int HideDelayMilliseconds = 1200;

        private readonly Timer _hideTimer;
        private string _text = string.Empty;

        public HintForm()
        {
            FormBorderStyle = FormBorderStyle.None;
            StartPosition = FormStartPosition.Manual;
            ShowInTaskbar = false;
            TopMost = true;
            DoubleBuffered = true;
            BackColor = SystemColors.Info;
            ForeColor = SystemColors.InfoText;

            _hideTimer = new Timer { Interval = HideDelayMilliseconds };
            _hideTimer.Tick += OnHideTimerTick;
        }

        protected override bool ShowWithoutActivation => true;

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams cp = base.CreateParams;
                cp.ExStyle |= WsExNoActivate | WsExToolWindow;
                return cp;
            }
        }

        // Shows a transient hint above the anchor, or below it when there is no room.
        public void ShowAbove(string text, Rectangle anchorScreenBounds)
        {
            SetTextAndSize(text);

            Rectangle screen = Screen.FromRectangle(anchorScreenBounds).WorkingArea;
            int gap = Dpi.Scale(4);
            int x = anchorScreenBounds.Left + ((anchorScreenBounds.Width - Width) / 2);

            int y = anchorScreenBounds.Top - Height - gap;
            if (y < screen.Top)
            {
                y = anchorScreenBounds.Bottom + gap;
            }

            if (y + Height > screen.Bottom)
            {
                y = Math.Max(screen.Top, anchorScreenBounds.Top - Height - gap);
            }

            x = Math.Max(screen.Left, Math.Min(x, screen.Right - Width));
            ShowAt(x, y);

            _hideTimer.Stop();
            _hideTimer.Start();
        }

        // Shows a hint next to the cursor and keeps it visible until HideHint is called.
        public void ShowNearCursor(string text, Point cursorScreenPoint)
        {
            SetTextAndSize(text);

            Rectangle screen = Screen.FromPoint(cursorScreenPoint).WorkingArea;
            int offset = Dpi.Scale(16);
            int x = Math.Min(cursorScreenPoint.X + offset, screen.Right - Width);
            int y = Math.Min(cursorScreenPoint.Y + offset, screen.Bottom - Height);
            x = Math.Max(screen.Left, x);
            y = Math.Max(screen.Top, y);
            ShowAt(x, y);

            _hideTimer.Stop();
        }

        public void HideHint()
        {
            _hideTimer.Stop();
            if (Visible)
            {
                Hide();
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);

            int paddingX = Dpi.Scale(6);
            int paddingY = Dpi.Scale(3);
            var textBounds = new Rectangle(
                paddingX,
                paddingY,
                ClientSize.Width - (paddingX * 2),
                ClientSize.Height - (paddingY * 2));

            TextRenderer.DrawText(
                e.Graphics,
                _text,
                Font,
                textBounds,
                ForeColor,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine);

            using (var borderPen = new Pen(SystemColors.ActiveBorder))
            {
                e.Graphics.DrawRectangle(borderPen, 0, 0, Width - 1, Height - 1);
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _hideTimer.Stop();
                _hideTimer.Dispose();
            }

            base.Dispose(disposing);
        }

        private void SetTextAndSize(string text)
        {
            _text = text;

            int paddingX = Dpi.Scale(6);
            int paddingY = Dpi.Scale(3);
            Size textSize = TextRenderer.MeasureText(
                text,
                Font,
                new Size(10000, 10000),
                TextFormatFlags.SingleLine);
            ClientSize = new Size(
                textSize.Width + (paddingX * 2),
                textSize.Height + (paddingY * 2));
        }

        private void ShowAt(int x, int y)
        {
            Location = new Point(x, y);
            if (!Visible)
            {
                Show();
            }
            else
            {
                BringToFront();
            }

            Invalidate();
        }

        private void OnHideTimerTick(object sender, EventArgs e)
        {
            _hideTimer.Stop();
            Hide();
        }
    }
}
