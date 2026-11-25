import React, { useEffect, useState } from 'react';
import { View } from 'react-native';
import { AppText, BackLink, Card, Icon, Page, palette, type } from './ui';
import { userFacingError } from '../utils/errors';

export function AuthFrame({ title, subtitle, children }: { title: string; subtitle: string; children: React.ReactNode }) { return <Page narrow><BackLink /><View style={{ alignItems: 'center', gap: 12 }}><View style={{ height: 52, width: 52, borderRadius: 17, backgroundColor: palette.pale, alignItems: 'center', justifyContent: 'center' }}><Icon name="activity" color={palette.green} size={25} /></View><AppText accessibilityRole="header" style={[type.title, { textAlign: 'center' }]}>{title}</AppText><AppText style={{ color: palette.muted, textAlign: 'center', maxWidth: 430 }}>{subtitle}</AppText></View><Card style={{ width: '100%', maxWidth: 460, alignSelf: 'center', padding: 25, gap: 19 }}>{children}</Card><AppText style={{ textAlign: 'center', color: palette.muted, fontSize: 11 }}>Một tài khoản. Nhiều buổi chơi đáng nhớ.</AppText></Page>; }
export function useCooldown() { const [remaining, setRemaining] = useState(0); useEffect(() => { if (remaining <= 0) return; const timer = setTimeout(() => setRemaining((current) => Math.max(0, current - 1)), 1000); return () => clearTimeout(timer); }, [remaining]); return { remaining, restart: () => setRemaining(60) }; }
export function formError(error: unknown) { return userFacingError(error); }
