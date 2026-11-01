using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;

namespace AmaraMusicVisualiser
{
    public static class WindowsAudioCapture
    {
        private const int S_OK = 0;
        private const int S_FALSE = 1;
        private const int RPC_E_CHANGED_MODE = unchecked((int)0x80010106);
        private const uint CLSCTX_ALL = 23;
        private const uint AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000;
        private const uint AUDCLNT_BUFFERFLAGS_SILENT = 0x2;
        private const ushort WAVE_FORMAT_PCM = 1;
        private const ushort WAVE_FORMAT_IEEE_FLOAT = 3;
        private const ushort WAVE_FORMAT_EXTENSIBLE = 0xfffe;
        private const int OutputRate = 48000;
        private static readonly Guid PcmSubFormat = new Guid("00000001-0000-0010-8000-00aa00389b71");
        private static readonly Guid FloatSubFormat = new Guid("00000003-0000-0010-8000-00aa00389b71");

        public static int Run()
        {
            int comResult = CoInitializeEx(IntPtr.Zero, 0);
            bool uninitialise = comResult == S_OK || comResult == S_FALSE;
            if (comResult < 0 && comResult != RPC_E_CHANGED_MODE) Marshal.ThrowExceptionForHR(comResult);
            IMMDeviceEnumerator enumerator = null;
            IMMDevice device = null;
            IAudioClient client = null;
            IAudioCaptureClient capture = null;
            IntPtr formatPointer = IntPtr.Zero;
            try
            {
                enumerator = (IMMDeviceEnumerator)new MMDeviceEnumeratorComObject();
                int endpointResult = enumerator.GetDefaultAudioEndpoint(EDataFlow.Render, ERole.Multimedia, out device);
                if (endpointResult < 0)
                    Check(enumerator.GetDefaultAudioEndpoint(EDataFlow.Render, ERole.Console, out device));
                Guid audioClientId = typeof(IAudioClient).GUID;
                object clientObject;
                Check(device.Activate(ref audioClientId, CLSCTX_ALL, IntPtr.Zero, out clientObject));
                client = (IAudioClient)clientObject;
                Check(client.GetMixFormat(out formatPointer));
                WaveFormatEx format = (WaveFormatEx)Marshal.PtrToStructure(formatPointer, typeof(WaveFormatEx));
                FormatKind kind = ReadFormatKind(formatPointer, format);
                if (format.nChannels < 1 || format.nSamplesPerSec < 8000 || format.nBlockAlign < 1)
                    throw new InvalidOperationException("The default Windows output format is invalid.");
                Check(client.Initialize(AudioClientShareMode.Shared, AUDCLNT_STREAMFLAGS_LOOPBACK,
                    1000000, 0, formatPointer, IntPtr.Zero));
                Guid captureClientId = typeof(IAudioCaptureClient).GUID;
                object captureObject;
                Check(client.GetService(ref captureClientId, out captureObject));
                capture = (IAudioCaptureClient)captureObject;
                Check(client.Start());
                Stream output = Console.OpenStandardOutput();
                LinearResampler resampler = new LinearResampler((int)format.nSamplesPerSec, OutputRate);
                Stopwatch clock = Stopwatch.StartNew();
                long lastEmission = clock.ElapsedTicks;
                int silentFrames = Math.Max(1, (int)format.nSamplesPerSec / 100);
                while (true)
                {
                    uint packetFrames;
                    Check(capture.GetNextPacketSize(out packetFrames));
                    if (packetFrames == 0)
                    {
                        if ((clock.ElapsedTicks - lastEmission) * 1000 / Stopwatch.Frequency >= 10)
                        {
                            resampler.Write(new float[silentFrames], output);
                            output.Flush();
                            lastEmission = clock.ElapsedTicks;
                        }
                        Thread.Sleep(2);
                        continue;
                    }
                    while (packetFrames > 0)
                    {
                        IntPtr data;
                        uint frames;
                        uint flags;
                        ulong devicePosition;
                        ulong qpcPosition;
                        Check(capture.GetBuffer(out data, out frames, out flags, out devicePosition, out qpcPosition));
                        try
                        {
                            float[] mono = (flags & AUDCLNT_BUFFERFLAGS_SILENT) != 0
                                ? new float[frames]
                                : DecodeMono(data, checked((int)frames), format, kind);
                            resampler.Write(mono, output);
                            lastEmission = clock.ElapsedTicks;
                        }
                        finally
                        {
                            Check(capture.ReleaseBuffer(frames));
                        }
                        Check(capture.GetNextPacketSize(out packetFrames));
                    }
                    output.Flush();
                }
            }
            catch (IOException)
            {
                return 0;
            }
            finally
            {
                if (client != null) client.Stop();
                if (formatPointer != IntPtr.Zero) Marshal.FreeCoTaskMem(formatPointer);
                Release(capture);
                Release(client);
                Release(device);
                Release(enumerator);
                if (uninitialise) CoUninitialize();
            }
        }

        private static float[] DecodeMono(IntPtr data, int frames, WaveFormatEx format, FormatKind kind)
        {
            int channels = format.nChannels;
            int bytesPerSample = format.nBlockAlign / channels;
            byte[] bytes = new byte[checked(frames * format.nBlockAlign)];
            Marshal.Copy(data, bytes, 0, bytes.Length);
            float[] mono = new float[frames];
            for (int frame = 0; frame < frames; frame++)
            {
                double sum = 0;
                int frameOffset = frame * format.nBlockAlign;
                for (int channel = 0; channel < channels; channel++)
                {
                    int offset = frameOffset + channel * bytesPerSample;
                    sum += DecodeSample(bytes, offset, bytesPerSample, format.wBitsPerSample, kind);
                }
                double value = sum / channels;
                mono[frame] = (float)Math.Max(-1, Math.Min(1, value));
            }
            return mono;
        }

        private static double DecodeSample(byte[] bytes, int offset, int bytesPerSample, int validBits, FormatKind kind)
        {
            if (kind == FormatKind.Float && bytesPerSample == 4) return BitConverter.ToSingle(bytes, offset);
            if (kind != FormatKind.Pcm) throw new InvalidOperationException("The Windows output format is not PCM or float.");
            if (bytesPerSample == 2) return BitConverter.ToInt16(bytes, offset) / 32768.0;
            if (bytesPerSample == 3)
            {
                int value = bytes[offset] | bytes[offset + 1] << 8 | bytes[offset + 2] << 16;
                if ((value & 0x800000) != 0) value |= unchecked((int)0xff000000);
                return value / 8388608.0;
            }
            if (bytesPerSample == 4)
            {
                int value = BitConverter.ToInt32(bytes, offset);
                int bits = Math.Max(1, Math.Min(32, validBits));
                if (bits < 32) value >>= 32 - bits;
                return value / Math.Pow(2, bits - 1);
            }
            throw new InvalidOperationException("The Windows output sample width is unsupported.");
        }

        private static FormatKind ReadFormatKind(IntPtr pointer, WaveFormatEx format)
        {
            if (format.wFormatTag == WAVE_FORMAT_IEEE_FLOAT) return FormatKind.Float;
            if (format.wFormatTag == WAVE_FORMAT_PCM) return FormatKind.Pcm;
            if (format.wFormatTag != WAVE_FORMAT_EXTENSIBLE || format.cbSize < 22)
                throw new InvalidOperationException("The Windows output format is unsupported.");
            WaveFormatExtensible extended = (WaveFormatExtensible)Marshal.PtrToStructure(pointer, typeof(WaveFormatExtensible));
            if (extended.SubFormat == FloatSubFormat) return FormatKind.Float;
            if (extended.SubFormat == PcmSubFormat) return FormatKind.Pcm;
            throw new InvalidOperationException("The Windows output subformat is unsupported.");
        }

        private static void Check(int result)
        {
            if (result < 0) Marshal.ThrowExceptionForHR(result);
        }

        private static void Release(object value)
        {
            if (value != null && Marshal.IsComObject(value)) Marshal.FinalReleaseComObject(value);
        }

        private enum FormatKind { Pcm, Float }

        private sealed class LinearResampler
        {
            private readonly double step;
            private readonly List<float> samples = new List<float>();
            private double position;

            public LinearResampler(int sourceRate, int outputRate)
            {
                step = (double)sourceRate / outputRate;
            }

            public void Write(float[] input, Stream output)
            {
                samples.AddRange(input);
                List<float> converted = new List<float>((int)(input.Length / step) + 2);
                while (position + 1 < samples.Count)
                {
                    int index = (int)position;
                    double fraction = position - index;
                    float a = samples[index];
                    float b = samples[index + 1];
                    converted.Add((float)(a + (b - a) * fraction));
                    position += step;
                }
                int consumed = Math.Min((int)position, Math.Max(0, samples.Count - 1));
                if (consumed > 0)
                {
                    samples.RemoveRange(0, consumed);
                    position -= consumed;
                }
                if (converted.Count == 0) return;
                float[] values = converted.ToArray();
                byte[] bytes = new byte[values.Length * 4];
                Buffer.BlockCopy(values, 0, bytes, 0, bytes.Length);
                output.Write(bytes, 0, bytes.Length);
            }
        }

        [DllImport("ole32.dll")]
        private static extern int CoInitializeEx(IntPtr reserved, uint coInit);
        [DllImport("ole32.dll")]
        private static extern void CoUninitialize();

        [StructLayout(LayoutKind.Sequential, Pack = 2)]
        private struct WaveFormatEx
        {
            public ushort wFormatTag;
            public ushort nChannels;
            public uint nSamplesPerSec;
            public uint nAvgBytesPerSec;
            public ushort nBlockAlign;
            public ushort wBitsPerSample;
            public ushort cbSize;
        }

        [StructLayout(LayoutKind.Sequential, Pack = 2)]
        private struct WaveFormatExtensible
        {
            public WaveFormatEx Format;
            public ushort ValidBitsPerSample;
            public uint ChannelMask;
            public Guid SubFormat;
        }

        private enum EDataFlow { Render, Capture, All }
        private enum ERole { Console, Multimedia, Communications }
        private enum AudioClientShareMode { Shared, Exclusive }

        [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
        private class MMDeviceEnumeratorComObject { }

        [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IMMDeviceEnumerator
        {
            [PreserveSig] int EnumAudioEndpoints(EDataFlow dataFlow, uint stateMask, out object devices);
            [PreserveSig] int GetDefaultAudioEndpoint(EDataFlow dataFlow, ERole role, out IMMDevice device);
            [PreserveSig] int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
            [PreserveSig] int RegisterEndpointNotificationCallback(IntPtr client);
            [PreserveSig] int UnregisterEndpointNotificationCallback(IntPtr client);
        }

        [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IMMDevice
        {
            [PreserveSig] int Activate(ref Guid interfaceId, uint classContext, IntPtr activationParameters,
                [MarshalAs(UnmanagedType.IUnknown)] out object instance);
            [PreserveSig] int OpenPropertyStore(uint access, out IntPtr properties);
            [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
            [PreserveSig] int GetState(out uint state);
        }

        [ComImport, Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IAudioClient
        {
            [PreserveSig] int Initialize(AudioClientShareMode shareMode, uint streamFlags, long bufferDuration,
                long periodicity, IntPtr format, IntPtr audioSessionGuid);
            [PreserveSig] int GetBufferSize(out uint bufferFrames);
            [PreserveSig] int GetStreamLatency(out long latency);
            [PreserveSig] int GetCurrentPadding(out uint paddingFrames);
            [PreserveSig] int IsFormatSupported(AudioClientShareMode shareMode, IntPtr format, out IntPtr closestMatch);
            [PreserveSig] int GetMixFormat(out IntPtr format);
            [PreserveSig] int GetDevicePeriod(out long defaultPeriod, out long minimumPeriod);
            [PreserveSig] int Start();
            [PreserveSig] int Stop();
            [PreserveSig] int Reset();
            [PreserveSig] int SetEventHandle(IntPtr eventHandle);
            [PreserveSig] int GetService(ref Guid interfaceId, [MarshalAs(UnmanagedType.IUnknown)] out object service);
        }

        [ComImport, Guid("C8ADBD64-E71E-48A0-A4DE-185C395CD317"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IAudioCaptureClient
        {
            [PreserveSig] int GetBuffer(out IntPtr data, out uint frames, out uint flags,
                out ulong devicePosition, out ulong qpcPosition);
            [PreserveSig] int ReleaseBuffer(uint frames);
            [PreserveSig] int GetNextPacketSize(out uint frames);
        }
    }
}
