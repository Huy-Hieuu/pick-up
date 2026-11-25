import React, { useState } from 'react';
import { Pressable, View } from 'react-native';
import { Redirect, router, useLocalSearchParams } from 'expo-router';
import { useSession } from '../../src/stores/session';
import { isSession } from '../../src/api/client';
import { AppText, Button, Field, Message, palette, safeReturn, ui } from '../../src/components/ui';
import { AuthFrame, formError } from '../../src/components/AuthFrame';

export default function Login() {
  const { next, reset } = useLocalSearchParams<{ next?: string; reset?: string }>();
  const { session, ready, request, authenticate } = useSession();
  const [email, setEmail] = useState(''); const [password, setPassword] = useState(''); const [visible, setVisible] = useState(false); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null);
  async function submit() {
    if (busy) return; setError(null);
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim()) || !password) { setError('Nhập email hợp lệ và mật khẩu để tiếp tục.'); return; }
    setBusy(true);
    try { const value = await request<unknown>('/auth/login', { method: 'POST', body: { email: email.trim().toLowerCase(), password } }); if (!isSession(value)) throw new Error('Phiên đăng nhập không hợp lệ. Vui lòng thử lại.'); await authenticate(value); router.replace(safeReturn(next)); }
    catch (reason) { setError(formError(reason)); } finally { setBusy(false); }
  }
  if (ready && session) return <Redirect href={safeReturn(next)} />;
  return <AuthFrame title="Chào bạn trở lại." subtitle="Đồng đội và những buổi chơi mới đang chờ bạn.">{reset === '1' && <Message>Đã đổi mật khẩu. Đăng nhập bằng mật khẩu mới.</Message>}{error && <Message error>{error}</Message>}<Field label="Email" placeholder="ban@example.com" value={email} onChangeText={setEmail} autoCapitalize="none" autoComplete="email" keyboardType="email-address" maxLength={254} editable={!busy} /><Field label="Mật khẩu" placeholder="Mật khẩu của bạn" value={password} onChangeText={setPassword} secureTextEntry={!visible} autoCapitalize="none" autoComplete="current-password" maxLength={128} editable={!busy} onSubmitEditing={() => { void submit(); }} /><View style={[ui.row, { justifyContent: 'space-between', flexWrap: 'wrap' }]}><Pressable accessibilityRole="button" onPress={() => setVisible((value) => !value)} style={ui.link}><AppText style={{ fontSize: 12, color: palette.muted }}>{visible ? 'Ẩn mật khẩu' : 'Hiện mật khẩu'}</AppText></Pressable><Pressable accessibilityRole="link" onPress={() => router.push('/reset-password')} style={ui.link}><AppText style={{ fontSize: 12 }}>Quên mật khẩu?</AppText></Pressable></View><Button title="Đăng nhập" loading={busy} onPress={() => { void submit(); }} /><View style={ui.divider} /><View style={{ alignItems: 'center', gap: 5 }}><AppText style={{ color: palette.muted }}>Bạn chưa có tài khoản?</AppText><Pressable accessibilityRole="link" onPress={() => router.push({ pathname: '/register', params: next ? { next } : {} })} style={ui.link}><AppText style={{ fontFamily: 'Inter_600SemiBold' }}>Tạo tài khoản ↗</AppText></Pressable></View></AuthFrame>;
}
