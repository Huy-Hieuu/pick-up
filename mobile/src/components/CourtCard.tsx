import React, { useState } from 'react';
import { Image } from 'expo-image';
import { Pressable, View } from 'react-native';
import { Link } from 'expo-router';
import type { Court } from '../types/api';
import { useSession } from '../stores/session';
import { money, safeWebUrl, sportName } from '../utils/format';
import { AppText, Icon, palette, type, ui } from './ui';
import { SportArtwork } from './SportArtwork';

export function CourtCard({ court, width }: { court: Court; width: number }) {
  const { saved, toggleSaved } = useSession();
  const bookmarked = saved.includes(court.id);
  const photo = court.photo_urls.map(safeWebUrl).find(Boolean);
  const [failedPhoto, setFailedPhoto] = useState<string | null>(null);
  const shownPhoto = photo && failedPhoto !== photo ? photo : undefined;
  return <View style={{ width, borderRadius: 18, backgroundColor: palette.white, borderWidth: 1, borderColor: palette.line, overflow: 'hidden' }}>
    <Link href={{ pathname: '/court/[id]', params: { id: court.id } }} accessibilityRole="link" accessibilityLabel={'Xem sân ' + court.name}>
      <View style={{ height: 190, backgroundColor: palette.pale, justifyContent: 'center', alignItems: 'center' }}>{shownPhoto ? <Image source={{ uri: shownPhoto }} contentFit="cover" transition={160} style={{ width: '100%', height: '100%' }} accessibilityLabel={court.name} onError={() => setFailedPhoto(shownPhoto)} /> : <SportArtwork sport={court.sport_type} />}<View style={{ position: 'absolute', bottom: 14, left: 14, backgroundColor: palette.white, paddingHorizontal: 11, paddingVertical: 5, borderRadius: 20 }}><AppText style={{ fontFamily: 'Inter_500Medium', fontSize: 11 }}>{sportName(court.sport_type)}</AppText></View></View>
      <View style={{ padding: 18, gap: 9 }}><AppText numberOfLines={2} style={type.subheading}>{court.name}</AppText><View style={ui.row}><Icon name="map-pin" size={14} /><AppText numberOfLines={2} style={{ flex: 1, fontSize: 12, color: palette.muted }}>{court.address}</AppText></View><View style={[ui.row, { justifyContent: 'space-between', marginTop: 6 }]}><AppText style={{ fontFamily: 'Inter_600SemiBold' }}>{money(court.price_per_slot)}<AppText style={{ color: palette.muted, fontSize: 11 }}> / khung giờ</AppText></AppText>{court.distance_km != null && <AppText style={{ fontSize: 11, color: palette.muted }}>{court.distance_km.toFixed(1)} km</AppText>}</View></View>
    </Link>
    <Pressable accessibilityRole="button" accessibilityLabel={(bookmarked ? 'Bỏ lưu sân ' : 'Lưu sân ') + court.name} accessibilityState={{ selected: bookmarked }} onPress={() => toggleSaved(court.id)} style={{ position: 'absolute', top: 12, right: 12, width: 48, height: 48, borderRadius: 24, backgroundColor: palette.white, alignItems: 'center', justifyContent: 'center' }}><Icon name="heart" color={bookmarked ? palette.danger : palette.ink} size={19} /></Pressable>
  </View>;
}
