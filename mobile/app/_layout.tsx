import React, { useEffect } from "react";
import { ActivityIndicator, View } from "react-native";
import { Stack, type ErrorBoundaryProps } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { useFonts } from "expo-font";
import { Inter_400Regular } from "@expo-google-fonts/inter/400Regular";
import { Inter_500Medium } from "@expo-google-fonts/inter/500Medium";
import { Inter_600SemiBold } from "@expo-google-fonts/inter/600SemiBold";
import { Inter_700Bold } from "@expo-google-fonts/inter/700Bold";
import * as SplashScreen from "expo-splash-screen";
import { SessionProvider, useSession } from "../src/stores/session";
import { AppShell } from "../src/components/AppShell";
import { AppText, Button, palette } from "../src/components/ui";

void SplashScreen.preventAutoHideAsync().catch(() => undefined);

function ConfiguredApp() {
  const { configurationError } = useSession();
  if (configurationError) {
    return (
      <View
        style={{
          flex: 1,
          backgroundColor: palette.paper,
          justifyContent: "center",
          padding: 28,
          gap: 16,
        }}
      >
        <AppText accessibilityRole="header" style={{ fontSize: 24 }}>
          Chưa thể mở PickUp
        </AppText>
        <AppText style={{ color: palette.muted }}>
          {configurationError.message}
        </AppText>
        <AppText style={{ color: palette.muted, fontSize: 12 }}>
          EXPO_PUBLIC_API_URL là cấu hình công khai, được nhúng khi build ứng dụng.
        </AppText>
      </View>
    );
  }
  return (
    <AppShell>
      <Stack
        screenOptions={{
          headerShown: false,
          contentStyle: { backgroundColor: palette.paper },
          animation: "fade",
        }}
      />
    </AppShell>
  );
}

export default function RootLayout() {
  const [loaded, error] = useFonts({
    Inter_400Regular,
    Inter_500Medium,
    Inter_600SemiBold,
    Inter_700Bold,
  });
  useEffect(() => {
    if (loaded || error) void SplashScreen.hideAsync().catch(() => undefined);
  }, [loaded, error]);
  if (!loaded && !error)
    return (
      <View
        style={{
          flex: 1,
          backgroundColor: palette.paper,
          justifyContent: "center",
        }}
      >
        <ActivityIndicator color={palette.green} />
      </View>
    );
  return (
    <SafeAreaProvider>
      <SessionProvider>
        <StatusBar style="dark" />
        <ConfiguredApp />
      </SessionProvider>
    </SafeAreaProvider>
  );
}
export function ErrorBoundary({ retry }: ErrorBoundaryProps) {
  return (
    <View
      style={{
        flex: 1,
        padding: 28,
        gap: 18,
        justifyContent: "center",
        backgroundColor: palette.paper,
      }}
    >
      <AppText style={{ fontSize: 22 }}>Ứng dụng gặp sự cố</AppText>
      <AppText>
        Vui lòng thử lại. Nếu sự cố vẫn xảy ra, mở lại ứng dụng.
      </AppText>
      <Button title="Thử lại" onPress={retry} />
    </View>
  );
}
