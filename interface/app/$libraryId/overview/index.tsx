import { useInfiniteQuery } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { useEffect, useMemo, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import {
	ExplorerItem,
	ObjectKind,
	ObjectKindKey,
	useLibraryContext,
	useLibraryQuery,
	useRspcLibraryContext
} from '@sd/client';
import { z } from '@sd/ui/src/forms';
import { useExplorerStore, useExplorerTopBarOptions } from '~/hooks';
import Explorer from '../Explorer';
import { SEARCH_PARAMS, useExplorerOrder } from '../Explorer/util';
import { usePageLayout } from '../PageLayout';
import { TopBarPortal } from '../TopBar/Portal';
import TopBarOptions from '../TopBar/TopBarOptions';
import Statistics from '../overview/Statistics';
import Categories from './Categories';

// TODO: Replace left hand type with Category enum type (doesn't exist yet)

// Map the category to the ObjectKind for searching
const SearchableCategories: Record<string, ObjectKindKey> = {
	Photos: 'Image',
	Videos: 'Video',
	Music: 'Audio',
	Documents: 'Document',
	Encrypted: 'Encrypted',
	Books: 'Book'
};

export type SearchArgs = z.infer<typeof SEARCH_PARAMS>;

export const Component = () => {
	const page = usePageLayout();
	const explorerStore = useExplorerStore();
	const ctx = useRspcLibraryContext();
	const { library } = useLibraryContext();
	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();
	const [pageScrollTop, setPageScrollTop] = useState<number>(0);

	const [selectedCategory, setSelectedCategory] = useState<string>('Recents');

	// TODO: integrate this into search query
	const recentFiles = useLibraryQuery([
		'search.paths',
		{
			order: { object: { dateAccessed: false } },
			take: 50
		}
	]);
	// this should be redundant once above todo is complete
	const canSearch = !!SearchableCategories[selectedCategory] || selectedCategory === 'Favorites';

	const kind = ObjectKind[SearchableCategories[selectedCategory] || 0] as number;

	const categories = useLibraryQuery(['categories.list']);

	const isFavoritesCategory = selectedCategory === 'Favorites';

	const selectedCategoryData = categories.data?.find((c) => c.name === selectedCategory);

	//Provides a better scroll experience for categories with a lot of items or no items
	const scrollTopValue = selectedCategoryData
		? selectedCategoryData.count === 0
			? 100
			: selectedCategoryData.count >= 50
			? 280
			: 150
		: 150;
	const categoriesPageScroll =
		pageScrollTop >= scrollTopValue
			? '!absolute translate-y-0 right-[8px]'
			: pageScrollTop >= 120
			? 'translate-y-[-46px]'
			: '';

	// TODO: Make a custom double click handler for directories to take users to the location explorer.
	// For now it's not needed because folders shouldn't show.
	const query = useInfiniteQuery({
		enabled: canSearch,
		queryKey: [
			'search.paths',
			{
				library_id: library.uuid,
				arg: {
					order: useExplorerOrder(),
					favorite: isFavoritesCategory ? true : undefined,
					...(explorerStore.layoutMode === 'media'
						? {
								kind: [5, 7].includes(kind)
									? [kind]
									: isFavoritesCategory
									? [5, 7]
									: [5, 7, kind]
						  }
						: { kind: isFavoritesCategory ? [] : [kind] })
				}
			}
		] as const,
		queryFn: ({ pageParam: cursor, queryKey }) =>
			ctx.client.query([
				'search.paths',
				{
					...queryKey[1].arg,
					cursor
				}
			]),
		getNextPageParam: (lastPage) => lastPage.cursor ?? undefined
	});

	const searchItems = useMemo(() => query.data?.pages?.flatMap((d) => d.items), [query.data]);

	let items: ExplorerItem[] = [];
	switch (selectedCategory) {
		case 'Recents':
			items = recentFiles.data?.items || [];
			break;
		default:
			if (canSearch) {
				items = searchItems || [];
			}
	}

	//we get the top page scroll value to use with the Categories component
	useEffect(() => {
		const pageCurrent = page?.ref?.current;
		if (pageCurrent) {
			pageCurrent.addEventListener('scroll', () => {
				setPageScrollTop(pageCurrent?.scrollTop);
			});
			return () => {
				pageCurrent?.removeEventListener('scroll', () => {
					setPageScrollTop(pageCurrent?.scrollTop);
				});
			};
		}
	}, [page?.ref]);

	return (
		<>
			<TopBarPortal
				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>
			<Statistics />
			<Categories
				selectedCategory={selectedCategory}
				setSelectedCategory={(category) => {
					setSelectedCategory(category);
					page?.ref?.current?.scrollTo({ top: 0 });
				}}
				categories={categories.data}
				categoriesClassName={categoriesPageScroll}
			/>
			<Explorer
				inspectorClassName="!pt-0 !fixed !top-[50px] !right-[10px] !w-[260px]"
				viewClassName="!pl-0 !pt-[0] !h-auto !overflow-visible"
				listViewHeadersClassName="!top-[65px] z-30"
				items={items}
				onLoadMore={query.fetchNextPage}
				hasNextPage={query.hasNextPage}
				isFetchingNextPage={query.isFetchingNextPage}
				scrollRef={page?.ref}
			/>
		</>
	);
};
