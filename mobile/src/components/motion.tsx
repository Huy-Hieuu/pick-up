import React, { useEffect, useRef, useState, type ReactNode } from 'react';
import { AccessibilityInfo, Animated, Easing, type AccessibilityProps, type StyleProp, type ViewStyle } from 'react-native';

export function useReducedMotion() {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    let active = true;
    AccessibilityInfo.isReduceMotionEnabled().then((value) => {
      if (active) setReduced(value);
    }).catch(() => undefined);
    const subscription = AccessibilityInfo.addEventListener('reduceMotionChanged', setReduced);
    return () => {
      active = false;
      subscription.remove();
    };
  }, []);
  return reduced;
}

export function MotionView({
  children,
  delay = 0,
  duration = 420,
  distance = 16,
  scaleFrom = 1,
  style,
  accessibilityLiveRegion,
}: {
  children: ReactNode;
  delay?: number;
  duration?: number;
  distance?: number;
  scaleFrom?: number;
  style?: StyleProp<ViewStyle>;
  accessibilityLiveRegion?: AccessibilityProps['accessibilityLiveRegion'];
}) {
  const reduced = useReducedMotion();
  const progress = useRef(new Animated.Value(reduced ? 1 : 0)).current;
  useEffect(() => {
    if (reduced) {
      progress.setValue(1);
      return;
    }
    progress.setValue(0);
    const animation = Animated.timing(progress, {
      toValue: 1,
      duration,
      delay,
      easing: Easing.out(Easing.cubic),
      useNativeDriver: true,
    });
    animation.start();
    return () => animation.stop();
  }, [delay, duration, progress, reduced]);
  return (
    <Animated.View
      accessibilityLiveRegion={accessibilityLiveRegion}
      style={[
        style,
        {
          opacity: progress,
          transform: [
            { translateY: progress.interpolate({ inputRange: [0, 1], outputRange: [distance, 0] }) },
            { scale: progress.interpolate({ inputRange: [0, 1], outputRange: [scaleFrom, 1] }) },
          ],
        },
      ]}
    >
      {children}
    </Animated.View>
  );
}

export function RouteTransition({ children, routeKey }: { children: ReactNode; routeKey: string }) {
  const reduced = useReducedMotion();
  const progress = useRef(new Animated.Value(reduced ? 1 : 0)).current;
  useEffect(() => {
    if (reduced) {
      progress.setValue(1);
      return;
    }
    progress.setValue(0);
    const animation = Animated.timing(progress, {
      toValue: 1,
      duration: 320,
      easing: Easing.out(Easing.cubic),
      useNativeDriver: true,
    });
    animation.start();
    return () => animation.stop();
  }, [progress, reduced, routeKey]);
  return (
    <Animated.View
      style={{
        flex: 1,
        minHeight: 0,
        opacity: progress,
        transform: [{ translateY: progress.interpolate({ inputRange: [0, 1], outputRange: [10, 0] }) }],
      }}
    >
      {children}
    </Animated.View>
  );
}

export function usePressScale() {
  const reduced = useReducedMotion();
  const scale = useRef(new Animated.Value(1)).current;
  const pressIn = () => {
    if (reduced) return;
    Animated.spring(scale, { toValue: 0.97, speed: 45, bounciness: 0.08, useNativeDriver: true }).start();
  };
  const pressOut = () => {
    if (reduced) return;
    Animated.spring(scale, { toValue: 1, speed: 32, bounciness: 0.12, useNativeDriver: true }).start();
  };
  return { scale, pressIn, pressOut };
}

export function ActivePill({ selected, radius = 25, color }: { selected: boolean; radius?: number; color: string }) {
  const reduced = useReducedMotion();
  const progress = useRef(new Animated.Value(selected ? 1 : 0)).current;
  useEffect(() => {
    const animation = Animated.spring(progress, {
      toValue: selected ? 1 : 0,
      speed: selected ? 24 : 30,
      bounciness: selected ? 0.18 : 0,
      useNativeDriver: true,
    });
    if (reduced) progress.setValue(selected ? 1 : 0);
    else animation.start();
    return () => animation.stop();
  }, [progress, reduced, selected]);
  return (
    <Animated.View
      pointerEvents="none"
      style={{
        position: 'absolute',
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
        borderRadius: radius,
        backgroundColor: color,
        opacity: progress,
        transform: [{ scale: progress.interpolate({ inputRange: [0, 1], outputRange: [0.92, 1] }) }],
      }}
    />
  );
}
