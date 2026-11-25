import React, { useCallback, useState } from 'react';
import { View, useWindowDimensions } from 'react-native';
import { useFocusEffect } from 'expo-router';
import * as Location from 'expo-location';
import type { Court, Page as ApiPage, Sport } from '../types/api';
import { useResource } from '../hooks/useResource';
import { AppText, Button, Message, Pill, SectionTitle, StateBlock, palette, ui } from './ui';
import { CourtCard } from './CourtCard';
import { isCourtPage } from '../types/validators';
import { userFacingError } from '../utils/errors';

export function CourtListing() {
  const { width } = useWindowDimensions();
  const [sport, setSport] = useState<Sport | null>(null);
  const [page, setPage] = useState(1);
  const [near, setNear] = useState<{ lat: number; lng: number } | null>(null);
  const [locating, setLocating] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const query = '/courts?page=' + page + '&per_page=12' + (sport ? '&sport_type=' + sport : '') + (near ? '&lat=' + near.lat + '&lng=' + near.lng + '&radius_km=15' : '');
  const resource = useResource<ApiPage<Court>>(query, isCourtPage);
  useFocusEffect(useCallback(() => { resource.refresh(); }, [resource.refresh]));
  const courts = Array.isArray(resource.data?.items) ? resource.data.items : [];
  const columns = width >= 1000 ? 3 : width >= 670 ? 2 : 1;
  const cardWidth = (Math.min(width, 1240) - (width < 600 ? 40 : 80) - (columns - 1) * 20) / columns;
  async function locate() {
    if (near) { setNear(null); setPage(1); return; }
    setLocating(true); setNotice(null);
    try {
      const permission = await Location.requestForegroundPermissionsAsync();
      if (permission.status !== 'granted') { setNotice('Cho phép vị trí để tìm sân trong bán kính 15 km. Bạn vẫn có thể xem tất cả sân.'); return; }
      const position = await Location.getCurrentPositionAsync({ accuracy: Location.Accuracy.Balanced });
      setNear({ lat: position.coords.latitude, lng: position.coords.longitude }); setPage(1);
    } catch { setNotice('Chưa xác định được vị trí. Hãy kiểm tra quyền vị trí và thử lại.'); }
    finally { setLocating(false); }
  }
  return <View style={{ gap: 22 }}>
    <SectionTitle title={near ? 'Sân gần bạn' : 'Chọn sân. Bắt đầu cuộc vui.'} subtitle="Một khung giờ tốt cho buổi chơi tiếp theo." />
    <View style={[ui.row, { justifyContent: 'space-between', flexWrap: 'wrap', gap: 14 }]}><View style={[ui.row, { flexWrap: 'wrap', flexShrink: 1, maxWidth: '100%' }]}>{[{ title: 'Tất cả môn', value: null }, { title: 'Pickleball', value: 'pickleball' }, { title: 'Bóng đá mini', value: 'mini_football' }].map((item) => <Pill key={item.title} title={item.title} selected={sport === item.value} onPress={() => { setSport(item.value as Sport | null); setPage(1); }} />)}</View><Button title={near ? 'Đang tìm quanh bạn' : 'Sân gần tôi'} secondary icon="navigation" loading={locating} onPress={() => { void locate(); }} style={{ minHeight: 48 }} /></View>
    {notice && <Message>{notice}</Message>}{resource.error && resource.data && <><Message error>{userFacingError(resource.error)}</Message><Button title="Thử tải lại" secondary onPress={resource.refresh} /></>}
    {!resource.data && (resource.loading || resource.error) ? <StateBlock loading={resource.loading} error={resource.error} retry={resource.refresh} /> : courts.length === 0 ? <StateBlock empty={near ? 'Chưa có sân phù hợp trong bán kính 15 km. Thử xem tất cả sân hoặc chọn môn khác.' : 'Chưa có sân cho bộ lọc này. Hãy thử môn thể thao khác.'} /> : <View style={ui.grid}>{courts.map((court) => <CourtCard key={court.id} court={court} width={cardWidth} />)}</View>}
    {!!resource.data?.total && <View style={[ui.row, { justifyContent: 'space-between', flexWrap: 'wrap' }]}><AppText style={{ color: palette.muted, fontSize: 12 }}>{resource.data.total} sân · Trang {page}</AppText><View style={ui.row}><Button title="Trang trước" secondary disabled={page === 1 || resource.loading} onPress={() => setPage((current) => current - 1)} /><Button title="Trang sau" secondary disabled={page * 12 >= resource.data.total || resource.loading} onPress={() => setPage((current) => current + 1)} /></View></View>}
  </View>;
}
