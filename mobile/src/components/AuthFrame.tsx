import React, { useEffect, useState } from 'react';
import { Image } from 'expo-image';
import { View, useWindowDimensions } from 'react-native';
import { AppText, BackLink, Card, Icon, Page, palette, type } from './ui';
import { userFacingError } from '../utils/errors';

export function AuthFrame({ title, subtitle, children }: { title: string; subtitle: string; children: React.ReactNode }) {
  const { width } = useWindowDimensions();
  const wide = width >= 850;
  if (wide) {
    return (
      <Page>
        <BackLink />
        <View style={{ borderRadius: 28, overflow: 'hidden', borderWidth: 1, borderColor: palette.line, backgroundColor: palette.white, flexDirection: 'row', minHeight: 610 }}>
          <View style={{ width: '44%', backgroundColor: palette.forest, padding: 38, gap: 24, justifyContent: 'space-between' }}>
            <View style={{ gap: 16 }}>
              <AppText style={[type.label, { color: palette.lime }]}>PICKUP COMMUNITY</AppText>
              <AppText style={{ fontFamily: 'Inter_700Bold', fontSize: 48, lineHeight: 56, letterSpacing: -1.8, color: palette.white }}>
                Ra sân.{'\n'}
                <AppText style={{ fontFamily: 'Inter_700Bold', fontSize: 48, lineHeight: 56, color: palette.lime }}>Nhập hội.</AppText>
              </AppText>
              <AppText style={{ color: '#DDE7DA', fontSize: 14, lineHeight: 23, maxWidth: 340 }}>
                Một tài khoản để đặt sân, tìm đồng đội và chia tiền minh bạch cho mọi buổi chơi.
              </AppText>
            </View>
            <View style={{ height: 230, borderRadius: 20, overflow: 'hidden', backgroundColor: palette.green }}>
              <Image source={require('../../assets/pickleball-hero.webp')} contentFit="cover" contentPosition="center" style={{ width: '100%', height: '100%' }} accessibilityLabel="Cùng nhau chơi pickleball tại thành phố Hồ Chí Minh" />
            </View>
            <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 18 }}>
              {[{ icon: 'calendar', text: 'Đặt sân nhanh' }, { icon: 'users', text: 'Tìm đồng đội' }, { icon: 'check-circle', text: 'Chia tiền rõ ràng' }].map((item) => (
                <View key={item.text} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
                  <Icon name={item.icon as 'calendar' | 'users' | 'check-circle'} color={palette.lime} size={17} />
                  <AppText style={{ color: '#E6EEE1', fontSize: 12 }}>{item.text}</AppText>
                </View>
              ))}
            </View>
          </View>
          <View style={{ flex: 1, padding: 46, justifyContent: 'center', gap: 22 }}>
            <View style={{ gap: 10 }}>
              <AppText accessibilityRole="header" style={type.title}>{title}</AppText>
              <AppText style={{ color: palette.muted, maxWidth: 430 }}>{subtitle}</AppText>
            </View>
            <Card style={{ width: '100%', maxWidth: 460, alignSelf: 'center', padding: 27, gap: 19 }}>{children}</Card>
            <AppText style={{ textAlign: 'center', color: palette.muted, fontSize: 11 }}>Một tài khoản. Nhiều buổi chơi đáng nhớ.</AppText>
          </View>
        </View>
      </Page>
    );
  }
  return (
    <Page narrow>
      <BackLink />
      <View style={{ alignItems: 'center', gap: 12 }}>
        <View style={{ height: 52, width: 52, borderRadius: 17, backgroundColor: palette.pale, alignItems: 'center', justifyContent: 'center' }}><Icon name="activity" color={palette.green} size={25} /></View>
        <AppText accessibilityRole="header" style={[type.title, { textAlign: 'center' }]}>{title}</AppText>
        <AppText style={{ color: palette.muted, textAlign: 'center', maxWidth: 430 }}>{subtitle}</AppText>
      </View>
      <Card style={{ width: '100%', maxWidth: 460, alignSelf: 'center', padding: 25, gap: 19 }}>{children}</Card>
      <AppText style={{ textAlign: 'center', color: palette.muted, fontSize: 11 }}>Một tài khoản. Nhiều buổi chơi đáng nhớ.</AppText>
    </Page>
  );
}
export function useCooldown() { const [remaining, setRemaining] = useState(0); useEffect(() => { if (remaining <= 0) return; const timer = setTimeout(() => setRemaining((current) => Math.max(0, current - 1)), 1000); return () => clearTimeout(timer); }, [remaining]); return { remaining, restart: () => setRemaining(60) }; }
export function formError(error: unknown) { return userFacingError(error); }
