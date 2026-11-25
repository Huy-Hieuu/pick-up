import React, { useCallback, useState } from 'react';
import { View, useWindowDimensions } from 'react-native';
import { router, useFocusEffect } from 'expo-router';
import { useSession } from '../../src/stores/session';
import { useResource } from '../../src/hooks/useResource';
import type { GamePage, Sport } from '../../src/types/api';
import { AppText, AuthGate, Button, Page, Pill, StateBlock, palette, type, ui } from '../../src/components/ui';
import { GameCard } from '../../src/components/GameCard';
import { isGamePage } from '../../src/types/validators';

export default function Games() {
  const { session, ready } = useSession(); const { width } = useWindowDimensions();
  const [sport, setSport] = useState<Sport | null>(null); const [page, setPage] = useState(1);
  const resource = useResource<GamePage>(session ? '/games?status=open&page=' + page + '&per_page=12' + (sport ? '&sport_type=' + sport : '') : null, isGamePage);
  useFocusEffect(useCallback(() => { resource.refresh(); }, [resource.refresh]));
  const games = Array.isArray(resource.data?.games) ? resource.data.games : [];
  const columns = width >= 1000 ? 3 : width >= 670 ? 2 : 1;
  const cardWidth = (Math.min(width, 1240) - (width < 600 ? 40 : 80) - (columns - 1) * 20) / columns;
  return <Page><View style={{ gap: 12 }}><AppText style={ui.eyebrow}>MỖI TRẬN ĐẤU LÀ MỘT KẾT NỐI</AppText><AppText accessibilityRole="header" style={type.title}>Tìm đồng đội.{'\n'}Cùng ra sân.</AppText><AppText style={{ color: palette.muted }}>Chọn một trận đang tìm người để tham gia.</AppText></View>{!ready ? <StateBlock loading /> : !session ? <AuthGate next="/games">Đăng nhập để xem trận đấu, tham gia và chia tiền cùng đồng đội.</AuthGate> : <><View style={[ui.row, { flexWrap: 'wrap' }]}><Pill title="Tất cả môn" selected={!sport} onPress={() => { setSport(null); setPage(1); }} /><Pill title="Pickleball" selected={sport === 'pickleball'} onPress={() => { setSport('pickleball'); setPage(1); }} /><Pill title="Bóng đá mini" selected={sport === 'mini_football'} onPress={() => { setSport('mini_football'); setPage(1); }} /></View>{resource.error && resource.data && <StateBlock error={resource.error} retry={resource.refresh} />}{!resource.data && (resource.loading || resource.error) ? <StateBlock loading={resource.loading} error={resource.error} retry={resource.refresh} /> : games.length ? <View style={ui.grid}>{games.map((game) => <GameCard key={game.id} game={game} width={cardWidth} />)}</View> : <StateBlock empty="Chưa có trận đang tìm người cho môn này. Bạn có thể đặt sân và tạo trận mới." action={{ title: 'Khám phá sân', onPress: () => router.push('/explore') }} />}{!!resource.data?.total && <View style={[ui.row, { justifyContent: 'space-between', flexWrap: 'wrap' }]}><AppText style={{ color: palette.muted }}>Trang {page} · {resource.data.total} trận</AppText><View style={ui.row}><Button title="Trang trước" secondary disabled={page === 1 || resource.loading} onPress={() => setPage((current) => current - 1)} /><Button title="Trang sau" secondary disabled={page * 12 >= resource.data.total || resource.loading} onPress={() => setPage((current) => current + 1)} /></View></View>}</>}</Page>;
}
