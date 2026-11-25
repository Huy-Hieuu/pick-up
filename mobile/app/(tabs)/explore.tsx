import { AppText, BackLink, Page, type } from '../../src/components/ui';
import { CourtListing } from '../../src/components/CourtListing';
export default function ExploreCourts() { return <Page><BackLink /><AppText accessibilityRole="header" style={type.title}>Buổi chơi tiếp theo{'\n'}bắt đầu ở đây.</AppText><CourtListing /></Page>; }
