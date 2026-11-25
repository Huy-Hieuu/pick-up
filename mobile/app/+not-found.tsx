import { router } from 'expo-router';
import { AppText, Button, Page, type } from '../src/components/ui';
export default function NotFound() { return <Page narrow><AppText style={type.title}>Trang này không còn tồn tại.</AppText><AppText>Quay về khám phá để tìm sân và trận đấu mới.</AppText><Button title="Về trang chủ" onPress={() => router.replace('/')} /></Page>; }
