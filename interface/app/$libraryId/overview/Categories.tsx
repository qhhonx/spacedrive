import { getIcon } from '@sd/assets/util';
import { iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { ArrowLeft, ArrowRight } from 'phosphor-react';
import { Dispatch, SetStateAction, useEffect, useRef, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { CategoryItem } from '~/../packages/client/src';
import { useIsDark } from '~/hooks';
import CategoryButton from '../overview/CategoryButton';

interface Props {
	selectedCategory: string;
	setSelectedCategory: Dispatch<SetStateAction<string>>;
	categories: CategoryItem[] | undefined;
	categoriesClassName?: string;
}

const CategoryToIcon: Record<string, string> = {
	Recents: iconNames.Collection,
	Favorites: iconNames.HeartFlat,
	Photos: iconNames.Image,
	Videos: iconNames.Video,
	Movies: iconNames.Movie,
	Music: iconNames.Audio,
	Documents: iconNames.Document,
	Downloads: iconNames.Package,
	Applications: iconNames.Application,
	Games: iconNames.Game,
	Books: iconNames.Book,
	Encrypted: iconNames.EncryptedLock,
	Archives: iconNames.Database,
	Projects: iconNames.Folder,
	Trash: iconNames.Trash
};

export default ({
	categories,
	selectedCategory,
	setSelectedCategory,
	categoriesClassName
}: Props) => {
	const [categoriesScroll, setCategoriesScroll] = useState<number>(0);
	const categoriesRef = useRef<HTMLDivElement>(null);
	const isDark = useIsDark();

	useEffect(() => {
		const categoriesCurrent = categoriesRef.current;
		if (categoriesCurrent) {
			const scrollValueHandler = () => {
				setCategoriesScroll(categoriesCurrent.scrollLeft);
			};
			categoriesCurrent.addEventListener('scroll', scrollValueHandler);
			return () => {
				categoriesCurrent.removeEventListener('scroll', scrollValueHandler);
			};
		}
	}, []);

	const arrowsHandler = (direction: 'right' | 'left') => {
		if (categoriesRef.current) {
			categoriesRef.current.scrollTo({
				left:
					direction === 'left'
						? categoriesRef.current.scrollLeft + 170
						: categoriesRef.current.scrollLeft - 170,
				behavior: 'smooth'
			});
		}
	};

	return (
		<div
			ref={categoriesRef}
			className={clsx(
				categoriesClassName,
				'no-scrollbar z-20 ml-[-14px] flex min-h-[80px] space-x-[1px] overflow-x-scroll bg-app/90 py-3 pr-5 backdrop-blur-md transition-all duration-200'
			)}
		>
			<div
				onClick={() => arrowsHandler('right')}
				className={clsx(
					categoriesScroll > 0
						? 'cursor-pointer bg-opacity-50 opacity-100 hover:opacity-80'
						: 'pointer-events-none',
					'sticky left-[33px] z-20 mt-2 flex h-9 w-9 items-center justify-center rounded-full border border-app-line bg-app p-2 opacity-0 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowLeft weight="bold" className="w-4 h-4 text-white" />
			</div>
			{categories?.map((category) => {
				const iconString = CategoryToIcon[category.name] || 'Document';
				return (
					<CategoryButton
						key={category.name}
						category={category.name}
						icon={getIcon(iconString, isDark)}
						items={category.count}
						selected={selectedCategory === category.name}
						onClick={() => {
							setSelectedCategory(category.name);
						}}
					/>
				);
			})}
			<div
				onClick={() => arrowsHandler('left')}
				className={clsx(
					categoriesScroll >= 570
						? 'pointer-events-none opacity-0 hover:opacity-0'
						: 'hover:opacity-80',
					'sticky right-[2px] z-20 mt-2 flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border border-app-line bg-app bg-opacity-50 p-2 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowRight weight="bold" className="w-4 h-4 text-white" />
			</div>
		</div>
	);
};
