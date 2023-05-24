import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { ArrowLeft, ArrowRight } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category, useLibraryQuery } from '@sd/client';
import { useIsDark } from '~/hooks';
import { usePageLayout } from '../PageLayout';
import CategoryButton from '../overview/CategoryButton';
import { IconForCategory } from './data';

const CategoryList = [
	'Recents',
	'Favorites',
	'Photos',
	'Videos',
	'Movies',
	'Music',
	'Documents',
	'Downloads',
	'Encrypted',
	'Projects',
	'Applications',
	'Archives',
	'Databases',
	'Games',
	'Books',
	'Contacts',
	'Trash'
] as Category[];

interface Props {
	selected: Category;
	onSelectedChanged(c: Category): void;
}

export const Categories = (props: Props) => {
	const categories = useLibraryQuery(['categories.list']);

	const [scroll, setScroll] = useState(0);
	const ref = useRef<HTMLDivElement>(null);

	const isDark = useIsDark();

	const page = usePageLayout();
	const [pageScrollTop, setPageScrollTop] = useState(0);

	useEffect(() => {
		const element = ref.current;

		if (!element) return;

		const handler = () => {
			setScroll(element.scrollLeft);
		};

		element.addEventListener('scroll', handler);
		return () => {
			element.removeEventListener('scroll', handler);
		};
	}, []);

	const handleArrowOnClick = (direction: 'right' | 'left') => {
		const element = ref.current;

		if (!element) return;

		element.scrollTo({
			left: direction === 'left' ? element.scrollLeft + 250 : element.scrollLeft - 250,
			behavior: 'smooth'
		});
	};

	const categoriesPageScroll = pageScrollTop > 10 ? '!translate-y-0' : '!translate-y-[85px]';

	// We get the top page scroll value to use with the Categories component
	useEffect(() => {
		const element = page?.ref?.current;

		if (!element) return;

		const handler = () => {
			setPageScrollTop(element?.scrollTop);
		};

		element.addEventListener('scroll', handler);

		return () => element?.removeEventListener('scroll', handler);
	}, [page?.ref, pageScrollTop]);

	return (
		<>
			<div
				ref={ref}
				className={clsx(
					categoriesPageScroll,
					'no-scrollbar absolute left-[-20px] z-20 flex min-h-[80px] w-full w-full translate-y-[85px] space-x-[1px] overflow-x-scroll bg-app/90 py-3 pr-5 backdrop-blur-md transition-all duration-500 ease-out'
				)}
			>
				<div className="sticky left-[15px] top-0 z-40 min-h-[40px] min-w-[40px] bg-gradient-to-r from-app" />
				<div
					onClick={() => handleArrowOnClick('right')}
					className={clsx(
						scroll > 0
							? 'cursor-pointer bg-opacity-50 opacity-100 hover:opacity-80'
							: 'pointer-events-none',
						'sticky left-[33px] z-40 mt-2 flex h-9 w-9 items-center justify-center rounded-full border border-app-line bg-app p-2 opacity-0 backdrop-blur-md transition-all duration-200'
					)}
				>
					<ArrowLeft weight="bold" className="w-4 h-4 text-white" />
				</div>
				{categories.data &&
					CategoryList.map((category) => {
						const iconString = IconForCategory[category] || 'Document';

						return (
							<CategoryButton
								key={category}
								category={category}
								icon={getIcon(iconString, isDark)}
								items={categories.data[category]}
								selected={props.selected === category}
								onClick={() => {
									props.onSelectedChanged(category);
									page?.ref?.current?.scrollTo({ top: 0 });
								}}
							/>
						);
					})}
				<div
					onClick={() => handleArrowOnClick('left')}
					className={clsx(
						scroll >= 1450
							? 'pointer-events-none opacity-0 hover:opacity-0'
							: 'hover:opacity-80',
						'sticky right-[2px] z-40 mt-2 flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border border-app-line bg-app bg-opacity-50 p-2 backdrop-blur-md transition-all duration-200'
					)}
				>
					<ArrowRight weight="bold" className="w-4 h-4 text-white" />
				</div>
				<div className="sticky right-[-20px] z-30 min-h-[40px] min-w-[100px] bg-gradient-to-l from-app" />
			</div>
		</>
	);
};
