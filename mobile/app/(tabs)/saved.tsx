import React, { useEffect, useState } from 'react';
import { View, useWindowDimensions } from 'react-native';
import { useSession } from '../../src/stores/session';
import type { Court } from '../../src/types/api';
import { AppText, Message, Page, StateBlock, type } from '../../src/components/ui';
import { CourtCard } from '../../src/components/CourtCard';
import { isCourt } from '../../src/types/validators';
import { ApiError } from '../../src/api/client';

export default function Saved() {
  const { saved, request, ready } = useSession(); const { width } = useWindowDimensions();
  const [courts, setCourts] = useState<Court[]>([]); const [loading, setLoading] = useState(true); const [error, setError] = useState<Error | null>(null); const [warning, setWarning] = useState<string | null>(null); const [version, setVersion] = useState(0);
  useEffect(() => { if (!ready) return; const controller = new AbortController(); let active = true; setLoading(true); setError(null); setWarning(null); Promise.allSettled(saved.map((id) => request<unknown>('/courts/' + encodeURIComponent(id), { signal: controller.signal }).then((value) => { if (!isCourt(value)) throw new ApiError(502, 'INVALID_RESPONSE', 'Máy chủ trả về dữ liệu không hợp lệ. Vui lòng thử lại.'); return value; }))).then((results) => { if (!active) return; const found = results.flatMap((result) => result.status === 'fulfilled' ? [result.value] : []); setCourts(found); const failedCount = results.length - found.length; if (failedCount) setWarning(`Không tải được ${failedCount} sân đã lưu. Các sân còn lại vẫn được hiển thị.`); if (saved.length && !found.length) { const failed = results.find((result) => result.status === 'rejected'); setError(failed?.status === 'rejected' && failed.reason instanceof Error ? failed.reason : new Error('Chưa tải được sân đã lưu.')); } }).finally(() => { if (active) setLoading(false); }); return () => { active = false; controller.abort(); }; }, [saved, ready, request, version]);
  const columns = width >= 1000 ? 3 : width >= 670 ? 2 : 1; const cardWidth = (Math.min(width, 1240) - (width < 600 ? 40 : 80) - (columns - 1) * 20) / columns;
  return <Page><AppText accessibilityRole="header" style={type.title}>Những sân bạn thích.</AppText><AppText>Sẵn sàng cho buổi chơi tiếp theo.</AppText>{warning && !loading && !error && <Message error>{warning}</Message>}{loading || error ? <StateBlock loading={loading} error={error} retry={() => setVersion((value) => value + 1)} /> : courts.length ? <View style={{ flexDirection: 'row', flexWrap: 'wrap', gap: 20 }}>{courts.map((court) => <CourtCard key={court.id} court={court} width={cardWidth} />)}</View> : <StateBlock empty="Chạm vào trái tim trên một sân để lưu vào danh sách của bạn." />}</Page>;
}
