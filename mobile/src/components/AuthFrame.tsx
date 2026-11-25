import React, { useEffect, useState } from 'react';
import { Image } from 'expo-image';
import { View, useWindowDimensions } from 'react-native';
import { AppText, BackLink, Card, Icon, Page, palette, type, ui } from './ui';
import { userFacingError } from '../utils/errors';
import { MotionView } from './motion';

const features = [
  { icon: 'calendar' as const, text: 'Đặt sân nhanh' },
  { icon: 'users' as const, text: 'Tìm đồng đội' },
  { icon: 'check-circle' as const, text: 'Chia tiền rõ ràng' },
];

export function AuthFrame({ title, subtitle, children }: { title: string; subtitle: string; children: React.ReactNode }) {
  const { width, height } = useWindowDimensions();
  const wide = width >= 850;
  const panelHeight = Math.min(610, Math.max(520, height - 250));

  if (wide) {
    return (
      <Page>
        <BackLink />
        <MotionView
          delay={30}
          duration={560}
          distance={18}
          style={{
            borderRadius: 30,
            overflow: 'hidden',
            borderWidth: 1,
            borderColor: palette.line,
            backgroundColor: palette.white,
            flexDirection: 'row',
            minHeight: panelHeight,
            boxShadow: '0 24px 70px rgba(25, 62, 50, 0.10)',
          }}
        >
          <MotionView
            delay={50}
            duration={620}
            distance={22}
            style={{ width: '46%', backgroundColor: palette.forest, padding: 30, justifyContent: 'space-between' }}
          >
            <Image
              source={require('../../assets/pickleball-hero.webp')}
              contentFit="cover"
              contentPosition="center"
              style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0, width: '100%', height: '100%' }}
              accessibilityLabel="Cùng nhau chơi pickleball tại thành phố Hồ Chí Minh"
            />
            <View style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0, backgroundColor: 'rgba(12, 38, 28, 0.76)' }} />
            <View style={{ gap: 15 }}>
              <View style={{ flexDirection: 'row', alignItems: 'center', gap: 9 }}>
                <View style={{ width: 7, height: 7, borderRadius: 4, backgroundColor: palette.lime }} />
                <AppText style={[type.label, { color: palette.lime }]}>PICKUP COMMUNITY</AppText>
              </View>
              <AppText style={{ fontFamily: 'Inter_700Bold', fontSize: 46, lineHeight: 52, letterSpacing: -1.7, color: palette.white }}>
                Ra sân.{'\n'}
                <AppText style={{ fontFamily: 'Inter_700Bold', fontSize: 46, lineHeight: 52, color: palette.lime }}>Nhập hội.</AppText>
              </AppText>
              <AppText style={{ color: '#E1E9DF', fontSize: 13, lineHeight: 21, maxWidth: 330 }}>
                Một tài khoản để đặt sân, tìm đồng đội và chia tiền minh bạch cho mọi buổi chơi.
              </AppText>
            </View>
            <View style={{ gap: 16 }}>
              <View style={{ gap: 12 }}>
                {features.map((item) => (
                  <View key={item.text} style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
                    <Icon name={item.icon} color={palette.lime} size={17} />
                    <AppText style={{ color: '#F0F5EC', fontSize: 12 }}>{item.text}</AppText>
                  </View>
                ))}
              </View>
              <View style={{ borderTopWidth: 1, borderTopColor: 'rgba(255,255,255,0.16)', paddingTop: 16, gap: 5 }}>
                <AppText style={{ color: palette.white, fontFamily: 'Inter_600SemiBold', fontSize: 13 }}>Ra sân cùng cộng đồng.</AppText>
                <AppText style={{ color: '#C9D7C5', fontSize: 11 }}>PickUp · Hồ Chí Minh</AppText>
              </View>
            </View>
          </MotionView>

          <MotionView
            delay={140}
            duration={560}
            distance={18}
            style={{ flex: 1, backgroundColor: palette.paper, padding: 20, justifyContent: 'center', gap: 16 }}
          >
            <View style={{ gap: 8 }}>
              <AppText style={[ui.eyebrow, { color: palette.green }]}>CHÀO MỪNG TRỞ LẠI</AppText>
              <AppText accessibilityRole="header" style={{ fontFamily: 'Inter_700Bold', fontSize: 32, lineHeight: 38, letterSpacing: -1, color: palette.ink }}>
                {title}
              </AppText>
              <AppText style={{ color: palette.muted, maxWidth: 430 }}>{subtitle}</AppText>
            </View>
            <Card style={{ width: '100%', maxWidth: 420, alignSelf: 'center', padding: 16, gap: 12, borderColor: 'transparent', boxShadow: '0 16px 44px rgba(25, 62, 50, 0.08)' }}>
              {children}
            </Card>
          </MotionView>
        </MotionView>
      </Page>
    );
  }

  return (
    <Page narrow>
      <BackLink />
      <MotionView delay={30} duration={480} distance={14} style={{ alignItems: 'center', gap: 12 }}>
        <View style={{ height: 52, width: 52, borderRadius: 17, backgroundColor: palette.pale, alignItems: 'center', justifyContent: 'center' }}>
          <Icon name="activity" color={palette.green} size={25} />
        </View>
        <AppText accessibilityRole="header" style={[type.title, { textAlign: 'center' }]}>{title}</AppText>
        <AppText style={{ color: palette.muted, textAlign: 'center', maxWidth: 430 }}>{subtitle}</AppText>
      </MotionView>
      <MotionView delay={110} duration={500} distance={18} style={{ width: '100%', maxWidth: 460, alignSelf: 'center' }}>
        <Card style={{ width: '100%', padding: 20, gap: 15, borderColor: 'transparent', boxShadow: '0 16px 44px rgba(25, 62, 50, 0.08)' }}>{children}</Card>
      </MotionView>
      <AppText style={{ textAlign: 'center', color: palette.muted, fontSize: 11 }}>Một tài khoản. Nhiều buổi chơi đáng nhớ.</AppText>
    </Page>
  );
}

export function useCooldown() {
  const [remaining, setRemaining] = useState(0);
  useEffect(() => {
    if (remaining <= 0) return;
    const timer = setTimeout(() => setRemaining((current) => Math.max(0, current - 1)), 1000);
    return () => clearTimeout(timer);
  }, [remaining]);
  return { remaining, restart: () => setRemaining(60) };
}

export function formError(error: unknown) {
  return userFacingError(error);
}
