using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;
using MicroTaskPager.Core;

namespace MicroTaskPager.GUI
{
    public class AboutDialog : Form
    {
        private readonly string _version =
            typeof(AboutDialog).Assembly.GetName().Version?.ToString(3)
            ?? Application.ProductVersion;

        public AboutDialog()
        {
            SuspendLayout();

            // Built in code, so tell WinForms to scale the pixel based layout from 96 DPI to the
            // system DPI (the process is system DPI aware, see app.manifest).
            AutoScaleDimensions = new SizeF(96F, 96F);
            AutoScaleMode = AutoScaleMode.Dpi;

            Text = "About Micro Task Pager";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            StartPosition = FormStartPosition.CenterScreen;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;
            ClientSize = new Size(600, 480);
            BackColor = Color.White;

            var iconBox = new PictureBox
            {
                Size = new Size(512, 278),
                SizeMode = PictureBoxSizeMode.Zoom,
                Location = new Point((ClientSize.Width - 512) / 2, 20),
            };

            try
            {
                using (var iconStream = new MemoryStream(
                    Helpers.GetResourceFromAssembly(typeof(Program), "MicroTaskPager.Resources.logo.png")))
                using (var pngImage = Image.FromStream(iconStream))
                {
                    iconBox.Image = new Bitmap(pngImage);
                }
            }
            catch
            {
                iconBox.Image = Icon?.ToBitmap();
            }

            var versionLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width,
                Height = 24,
                Location = new Point(0, 300),
                TextAlign = ContentAlignment.MiddleCenter,
                Font = new Font("Segoe UI", 10, FontStyle.Regular),
                ForeColor = Color.DimGray,
                Text = $"Version {_version}",
            };

            var authorLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width - 40,
                Height = 24,
                Location = new Point(20, 330),
                TextAlign = ContentAlignment.MiddleCenter,
                Font = new Font("Segoe UI", 9, FontStyle.Bold),
                Text = "Author: Robbson 2026",
            };

            var contributorsLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width - 40,
                Height = 60,
                Location = new Point((ClientSize.Width - 160) / 2, 360),
                TextAlign = ContentAlignment.TopLeft,
                Font = new Font("Segoe UI", 9, FontStyle.Regular),
                Text = "Acknowledgments:\n" +
                " • DARKGuy (original project)\n" +
                " • MScholtes (Virtual Desktop)",
            };

            var closeButton = new Button
            {
                Text = "Close",
                Width = 100,
                Height = 30,
                Location = new Point((ClientSize.Width - 100) / 2, 425),
                DialogResult = DialogResult.OK,
            };

            AcceptButton = closeButton;
            CancelButton = closeButton;

            Controls.Add(iconBox);
            Controls.Add(versionLabel);
            Controls.Add(authorLabel);
            Controls.Add(contributorsLabel);
            Controls.Add(closeButton);

            ResumeLayout(false);
        }
    }
}
