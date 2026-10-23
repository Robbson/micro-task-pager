using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;

namespace MicroTaskPager.GUI
{
    public partial class NumericUpDownControl : UserControl
    {
        // Register RouteEvent
        public static readonly RoutedEvent ValueChangedEvent =
            EventManager.RegisterRoutedEvent(
                nameof(ValueChanged),
                RoutingStrategy.Bubble,
                typeof(RoutedPropertyChangedEventHandler<int>),
                typeof(NumericUpDownControl));

        // Register of dependency property for the value
        public static readonly DependencyProperty ValueProperty =
            DependencyProperty.Register(
                nameof(Value),
                typeof(int),
                typeof(NumericUpDownControl),
                new FrameworkPropertyMetadata(
                    0,
                    FrameworkPropertyMetadataOptions.BindsTwoWayByDefault,
                    OnValueChanged,
                    CoerceValueCore));

        public static readonly DependencyProperty MinimumProperty =
            DependencyProperty.Register(
                nameof(Minimum),
                typeof(int),
                typeof(NumericUpDownControl),
                new PropertyMetadata(int.MinValue, OnLimitChanged));

        public static readonly DependencyProperty MaximumProperty =
            DependencyProperty.Register(
                nameof(Maximum),
                typeof(int),
                typeof(NumericUpDownControl),
                new PropertyMetadata(int.MaxValue, OnLimitChanged));

        public NumericUpDownControl() => InitializeComponent();

        // .NET wrapper for XAML access
        public event RoutedPropertyChangedEventHandler<int> ValueChanged
        {
            add => AddHandler(ValueChangedEvent, value);
            remove => RemoveHandler(ValueChangedEvent, value);
        }

        // .NET wrapper for XAML access
        public int Value
        {
            get => (int)GetValue(ValueProperty);
            set => SetValue(ValueProperty, value);
        }

        public int Minimum
        {
            get => (int)GetValue(MinimumProperty);
            set => SetValue(MinimumProperty, value);
        }

        public int Maximum
        {
            get => (int)GetValue(MaximumProperty);
            set => SetValue(MaximumProperty, value);
        }

        private static void OnLimitChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            if (d is NumericUpDownControl control)
            {
                control.CoerceValue(ValueProperty);
            }
        }

        private static object CoerceValueCore(DependencyObject d, object baseValue)
        {
            if (d is NumericUpDownControl control)
            {
                int value = (int)baseValue;
                int minimum = control.Minimum;
                int maximum = control.Maximum;

                return Math.Max(minimum, Math.Min(maximum, value));
            }

            return baseValue;
        }

        // Method gets called automatically when 'Value' changes
        private static void OnValueChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            if (d is NumericUpDownControl control)
            {
                int oldValue = (int)e.OldValue;
                int newValue = (int)e.NewValue;

                // Hier feuern wir das Event ab, das Sie im MainWindow abfangen können
                var args = new RoutedPropertyChangedEventArgs<int>(oldValue, newValue, ValueChangedEvent);
                control.RaiseEvent(args);
            }
        }

        // Hilfsmethode: Gibt true zurück, wenn der Text NUR aus Zahlen besteht
        private static bool IsTextNumeric(string text)
        {
            // Erlaubt nur Ziffern (0-9).
            // Wenn Sie auch negative Zahlen erlauben wollen, nutzen Sie: @"^-?[0-9]*$"
            return Regex.IsMatch(text, @"^[0-9]*$");
        }

        private void BtnDown_Click(object sender, RoutedEventArgs e)
        {
            CommitValue();
            Value--;
        }

        private void BtnUp_Click(object sender, RoutedEventArgs e)
        {
            CommitValue();
            Value++;
        }

        // 1. Tastatureingaben filtern
        private void TextBox_PreviewTextInput(object sender, System.Windows.Input.TextCompositionEventArgs e)
        {
            // Wenn der eingegebene Buchstabe KEINE Zahl ist, verwerfen wir das Event (Handled = true)
            e.Handled = !IsTextNumeric(e.Text);
        }

        // 2. Copy-Paste (Einfügen) filtern
        private void TextBox_Pasting(object sender, DataObjectPastingEventArgs e)
        {
            if (e.DataObject.GetDataPresent(typeof(string)))
            {
                string text = (string)e.DataObject.GetData(typeof(string));

                // Wenn der eingefügte Text Buchstaben enthält, brechen wir das Einfügen ab
                if (!IsTextNumeric(text))
                {
                    e.CancelCommand();
                }
            }
            else
            {
                // Falls jemand versucht, ein Bild oder eine Datei reinzukopieren
                e.CancelCommand();
            }
        }

        private void TextBox_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
        {
            if (e.Key == System.Windows.Input.Key.Space)
            {
                e.Handled = true; // Blockiert die Leertaste
            }
            else if (e.Key == System.Windows.Input.Key.Enter)
            {
                CommitValue();
                e.Handled = true;
            }
        }

        private void TextBox_LostFocus(object sender, RoutedEventArgs e) => CommitValue();

        private void CommitValue()
        {
            var binding = ValueTextBox.GetBindingExpression(TextBox.TextProperty);
            binding?.UpdateSource();
            binding?.UpdateTarget();
        }
    }
}
