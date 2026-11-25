import React, { useState } from 'react';
import { Pressable } from 'react-native';
import { Redirect, router, useLocalSearchParams } from 'expo-router';
import { useSession } from '../../src/stores/session';
import { isSession } from '../../src/api/client';
import { AppText, Button, Field, Message, palette, safeReturn, ui } from '../../src/components/ui';
import { AuthFrame, formError, useCooldown } from '../../src/components/AuthFrame';

export default function Register() {
  const { next } = useLocalSearchParams<{ next?: string }>(); const { session, ready, request, authenticate } = useSession();
  const [email, setEmail] = useState(''); const [sent, setSent] = useState(false); const [code, setCode] = useState(''); const [name, setName] = useState(''); const [phone, setPhone] = useState(''); const [password, setPassword] = useState(''); const [confirm, setConfirm] = useState(''); const [busy, setBusy] = useState(false); const [error, setError] = useState<string | null>(null); const cooldown = useCooldown();
  async function send(resend = false) {
    if (busy) return; setError(null);
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim())) { setError('Nhập email hợp lệ để nhận mã xác minh.'); return; }
    setBusy(true);
    try { await request('/auth/' + (resend ? 'resend-registration-otp' : 'register'), { method: 'POST', body: { email: email.trim().toLowerCase() } }); setSent(true); cooldown.restart(); }
    catch (reason) { setError(formError(reason)); } finally { setBusy(false); }
  }
  async function verify() {
    if (busy) return; setError(null); const normalizedPhone = phone.replace(/[\s()-]/g, '');
    if (!/^\d{6}$/.test(code)) { setError('Nhập đủ 6 chữ số trong email xác minh.'); return; }
    if (!/^(0|\+84)(3|5|7|8|9)\d{8}$/.test(normalizedPhone)) { setError('Nhập số điện thoại Việt Nam hợp lệ.'); return; }
    if (password.length < 8 || password.length > 128 || password !== confirm) { setError('Mật khẩu cần 8–128 ký tự và hai lần nhập phải khớp nhau.'); return; }
    setBusy(true);
    try { const value = await request<unknown>('/auth/verify-registration', { method: 'POST', body: { email: email.trim().toLowerCase(), code, phone: normalizedPhone, password, ...(name.trim() ? { display_name: name.trim() } : {}) } }); if (!isSession(value)) throw new Error('Phiên đăng nhập không hợp lệ. Vui lòng thử lại.'); await authenticate(value); router.replace(safeReturn(next)); }
    catch (reason) { setError(formError(reason)); } finally { setBusy(false); }
  }
  if (ready && session) return <Redirect href={safeReturn(next)} />;
  return <AuthFrame title={sent ? 'Sắp đến giờ ra sân.' : 'Gia nhập cuộc vui.'} subtitle={sent ? 'Nhập mã từ email và hoàn tất hồ sơ của bạn.' : 'Bắt đầu bằng email để tạo tài khoản PickUp.'}>{error && <Message error>{error}</Message>}<Field label="Email" value={email} onChangeText={setEmail} placeholder="ban@example.com" keyboardType="email-address" autoComplete="email" autoCapitalize="none" maxLength={254} editable={!sent && !busy} />{sent ? <><Message>Nếu email có thể đăng ký, mã xác minh sẽ được gửi tới {email.trim().toLowerCase()}. Hãy kiểm tra cả thư mục spam.</Message><Field label="Mã xác minh" value={code} onChangeText={(value) => setCode(value.replace(/\D/g, '').slice(0, 6))} keyboardType="number-pad" autoComplete="one-time-code" maxLength={6} placeholder="6 chữ số" editable={!busy} /><Field label="Tên hiển thị" value={name} onChangeText={setName} maxLength={50} placeholder="Bạn muốn đồng đội gọi mình là gì?" autoComplete="name" editable={!busy} /><Field label="Số điện thoại" value={phone} onChangeText={setPhone} keyboardType="phone-pad" autoComplete="tel" maxLength={20} placeholder="090 123 4567" editable={!busy} /><AppText style={{ fontSize: 11, color: palette.muted }}>Số liên hệ trong hồ sơ. Bạn đăng nhập và khôi phục tài khoản bằng email.</AppText><Field label="Mật khẩu" value={password} onChangeText={setPassword} secureTextEntry autoComplete="new-password" autoCapitalize="none" maxLength={128} placeholder="Ít nhất 8 ký tự" editable={!busy} /><Field label="Nhập lại mật khẩu" value={confirm} onChangeText={setConfirm} secureTextEntry autoComplete="new-password" autoCapitalize="none" maxLength={128} editable={!busy} /><Button title="Hoàn tất đăng ký" loading={busy} onPress={() => { void verify(); }} /><Button title={cooldown.remaining ? 'Gửi lại mã sau ' + cooldown.remaining + 's' : 'Gửi lại mã xác minh'} secondary disabled={!!cooldown.remaining || busy} onPress={() => { void send(true); }} /><Pressable accessibilityRole="button" onPress={() => { setSent(false); setCode(''); setError(null); }} style={ui.link}><AppText style={{ textAlign: 'center' }}>Đổi email</AppText></Pressable></> : <Button title="Gửi mã xác minh" loading={busy} onPress={() => { void send(); }} />}<Pressable accessibilityRole="link" onPress={() => router.replace({ pathname: '/login', params: next ? { next } : {} })} style={ui.link}><AppText style={{ textAlign: 'center', color: palette.muted }}>Đã có tài khoản? Đăng nhập ↗</AppText></Pressable></AuthFrame>;
}
